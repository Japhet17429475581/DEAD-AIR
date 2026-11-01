//! Bounds-checked 802.11/radiotap and EAPOL-Key metadata parser. No credentials
//! are derived. A correlated sequence is not proof of key possession.
use crate::model::{Exchange, Packet};
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct RawFrame {
    pub time: f64,
    pub data: Vec<u8>,
}
/// Read airodump-ng PCAP incrementally; tolerate a partially written tail.
pub fn read_pcap_record(
    file: &mut std::fs::File,
    offset: &mut u64,
) -> Result<Option<RawFrame>, String> {
    use std::io::{Read, Seek, SeekFrom};
    if file.metadata().map_err(|e| e.to_string())?.len() < 24 {
        return Ok(None);
    }
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut header = [0; 24];
    file.read_exact(&mut header).map_err(|e| e.to_string())?;
    let (little, nano) = match &header[..4] {
        [0xd4, 0xc3, 0xb2, 0xa1] => (true, false),
        [0xa1, 0xb2, 0xc3, 0xd4] => (false, false),
        [0x4d, 0x3c, 0xb2, 0xa1] => (true, true),
        [0xa1, 0xb2, 0x3c, 0x4d] => (false, true),
        _ => return Err("Unsupported airodump PCAP header".into()),
    };
    let number = |b: &[u8]| -> u32 {
        let a: [u8; 4] = b.try_into().unwrap();
        if little {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        }
    };
    let link = number(&header[20..24]);
    if link != 105 && link != 127 {
        return Err(format!("Unsupported capture link type {link}"));
    }
    if *offset < 24 {
        *offset = 24
    }
    let length = file.metadata().map_err(|e| e.to_string())?.len();
    if length < *offset + 16 {
        return Ok(None);
    }
    file.seek(SeekFrom::Start(*offset))
        .map_err(|e| e.to_string())?;
    let mut record = [0; 16];
    file.read_exact(&mut record).map_err(|e| e.to_string())?;
    let size = number(&record[8..12]) as usize;
    if size > 65535 {
        return Err("PCAP packet exceeds bounds".into());
    }
    if length < *offset + 16 + size as u64 {
        return Ok(None);
    }
    let mut data = vec![0; size];
    file.read_exact(&mut data).map_err(|e| e.to_string())?;
    *offset += 16 + size as u64;
    data.truncate(if link == 105 { 4088 } else { 4096 });
    if link == 105 {
        let mut wrapped = vec![0, 0, 8, 0, 0, 0, 0, 0];
        wrapped.extend(data);
        data = wrapped
    }
    Ok(Some(RawFrame {
        time: number(&record[0..4]) as f64
            + number(&record[4..8]) as f64 / if nano { 1e9 } else { 1e6 },
        data,
    }))
}
#[derive(Clone, Debug)]
pub struct KeyEvent {
    pub station: String,
    pub message: u8,
    pub replay: u64,
    pub nonce: [u8; 32],
    pub time: f64,
}
pub(crate) fn mac(b: &[u8]) -> String {
    b.iter()
        .map(|v| format!("{v:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}
pub(crate) fn radiotap(data: &[u8]) -> Option<(usize, Option<i8>)> {
    if data.len() < 8 || data[0] != 0 {
        return None;
    }
    let length = u16::from_le_bytes([data[2], data[3]]) as usize;
    if length < 8 || length > data.len() {
        return None;
    }
    let first = u32::from_le_bytes(data[4..8].try_into().ok()?);
    let mut cursor = 8;
    let mut present = first;
    while present & 0x80000000 != 0 {
        if cursor + 4 > length {
            return None;
        }
        present = u32::from_le_bytes(data[cursor..cursor + 4].try_into().ok()?);
        cursor += 4;
        if cursor > 128 {
            return None;
        }
    }
    let mut signal = None;
    for (bit, (alignment, size)) in [(8, 8), (1, 1), (1, 1), (2, 4), (2, 2), (1, 1)]
        .iter()
        .enumerate()
    {
        if first & (1 << bit) != 0 {
            cursor = (cursor + alignment - 1) / alignment * alignment;
            if cursor + size > length {
                return None;
            }
            if bit == 5 {
                signal = Some(data[cursor] as i8)
            }
            cursor += size
        }
    }
    Some((length, signal))
}
pub fn parse(data: &[u8], time: f64, bssid: [u8; 6]) -> Option<(Packet, Option<KeyEvent>)> {
    let (offset, signal) = radiotap(data)?;
    let frame = data.get(offset..)?;
    if frame.len() < 24 {
        return None;
    }
    let fc = u16::from_le_bytes([frame[0], frame[1]]);
    if fc & 3 != 0 {
        return None;
    }
    let kind = (fc >> 2) & 3;
    let subtype = (fc >> 4) & 15;
    let to_ds = fc & 0x100 != 0;
    let from_ds = fc & 0x200 != 0;
    let a1 = &frame[4..10];
    let a2 = &frame[10..16];
    let a3 = &frame[16..22];
    let (source, destination, target) = match (kind, to_ds, from_ds) {
        (0, _, _) => (a2, a1, a3),
        (2, true, false) => (a2, a3, a1),
        (2, false, true) => (a3, a1, a2),
        (2, false, false) => (a2, a1, a3),
        _ => return None,
    };
    if target != bssid {
        return None;
    }
    let station = if a2 == bssid { mac(a1) } else { mac(a2) };
    let mut packet = Packet {
        time,
        source: mac(source),
        destination: mac(destination),
        station: station.clone(),
        kind: String::new(),
        message: None,
        replay: None,
        signal,
        bytes: data.len(),
    };
    if kind == 0 {
        packet.kind = match subtype {
            0 => "Association request",
            1 => "Association response",
            2 => "Reassociation request",
            3 => "Reassociation response",
            4 => "Probe request",
            5 => "Probe response",
            8 => "Beacon",
            10 => "Disassociation observed",
            11 => "Authentication",
            12 => "Deauthentication observed",
            _ => "Management",
        }
        .into();
        return Some((packet, None));
    }
    // Reject protected data, A-MSDU aggregates, fragmentation and non-EAPOL.
    if fc & 0x4000 != 0 || fc & 0x400 != 0 || frame[22] & 15 != 0 {
        return None;
    }
    let mut header = 24;
    if subtype & 8 != 0 {
        if frame.len() < 26 || frame[24] & 0x80 != 0 {
            return None;
        }
        header += 2;
        if fc & 0x8000 != 0 {
            header += 4
        }
    }
    let llc = frame.get(header..header + 8)?;
    if llc != [0xaa, 0xaa, 0x03, 0, 0, 0, 0x88, 0x8e] {
        return None;
    }
    let e = frame.get(header + 8..)?;
    if e.len() < 99 || e[1] != 3 || !matches!(e[4], 2 | 254) {
        return None;
    }
    let declared = u16::from_be_bytes([e[2], e[3]]) as usize;
    if declared < 95 || declared + 4 > e.len() {
        return None;
    }
    let info = u16::from_be_bytes([e[5], e[6]]);
    if info & 8 == 0 || info & 0x0c00 != 0 {
        return None;
    }
    let ack = info & 0x80 != 0;
    let mic = info & 0x100 != 0;
    let secure = info & 0x200 != 0;
    let install = info & 0x40 != 0;
    let message = match (ack, mic, secure, install) {
        (true, false, false, false) => 1,
        (false, true, false, false) => 2,
        (true, true, _, true) => 3,
        (false, true, true, false) => 4,
        _ => return None,
    };
    // Direction must match the EAPOL message role.
    if (message == 1 || message == 3) != (a2 == bssid) {
        return None;
    }
    let replay = u64::from_be_bytes(e[9..17].try_into().ok()?);
    let nonce: [u8; 32] = e[17..49].try_into().ok()?;
    packet.kind = "EAPOL-Key".into();
    packet.message = Some(message);
    packet.replay = Some(replay.to_string());
    Some((
        packet,
        Some(KeyEvent {
            station,
            message,
            replay,
            nonce,
            time,
        }),
    ))
}
#[derive(Default)]
pub struct Tracker {
    events: BTreeMap<(String, u64, u8), KeyEvent>,
}
impl Tracker {
    pub fn observe(&mut self, event: KeyEvent) {
        if self.events.len() >= 512 {
            self.events.clear()
        }
        self.events
            .insert((event.station.clone(), event.replay, event.message), event);
    }
    pub fn exchanges(&self) -> Vec<Exchange> {
        let mut stations: BTreeMap<String, Exchange> = BTreeMap::new();
        for event in self.events.values() {
            let state = stations
                .entry(event.station.clone())
                .or_insert_with(|| Exchange {
                    station: event.station.clone(),
                    messages: vec![],
                    complete: false,
                });
            if !state.messages.contains(&event.message) {
                state.messages.push(event.message)
            }
            if event.message == 1 && event.nonce != [0; 32] {
                if let Some(m2) = self.events.get(&(event.station.clone(), event.replay, 2)) {
                    if m2.nonce == [0; 32] {
                        continue;
                    }
                    for m3 in self.events.values().filter(|x| {
                        x.station == event.station
                            && x.message == 3
                            && x.replay > event.replay
                            && x.nonce == event.nonce
                    }) {
                        if let Some(m4) = self.events.get(&(event.station.clone(), m3.replay, 4)) {
                            let times = [event.time, m2.time, m3.time, m4.time];
                            let min = times.iter().copied().fold(f64::INFINITY, f64::min);
                            let max = times.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                            if max - min <= 60.0 {
                                state.complete = true
                            }
                        }
                    }
                }
            }
        }
        stations
            .into_values()
            .map(|mut s| {
                s.messages.sort();
                s
            })
            .collect()
    }
}
pub fn pcapng(frames: &[RawFrame]) -> Vec<u8> {
    fn block(out: &mut Vec<u8>, kind: u32, body: Vec<u8>) {
        let length = (body.len() + 12) as u32;
        out.extend(kind.to_le_bytes());
        out.extend(length.to_le_bytes());
        out.extend(body);
        out.extend(length.to_le_bytes());
    }
    let mut out = Vec::new();
    let mut shb = Vec::new();
    shb.extend(0x1a2b3c4du32.to_le_bytes());
    shb.extend(1u16.to_le_bytes());
    shb.extend(0u16.to_le_bytes());
    shb.extend((-1i64).to_le_bytes());
    block(&mut out, 0x0a0d0d0a, shb);
    let mut idb = Vec::new();
    idb.extend(127u16.to_le_bytes());
    idb.extend(0u16.to_le_bytes());
    idb.extend(4096u32.to_le_bytes());
    block(&mut out, 1, idb);
    for frame in frames {
        let mut b = Vec::new();
        let micros = (frame.time * 1_000_000.0) as u64;
        b.extend(0u32.to_le_bytes());
        b.extend(((micros >> 32) as u32).to_le_bytes());
        b.extend((micros as u32).to_le_bytes());
        b.extend((frame.data.len() as u32).to_le_bytes());
        b.extend((frame.data.len() as u32).to_le_bytes());
        b.extend(&frame.data);
        while b.len() % 4 != 0 {
            b.push(0)
        }
        block(&mut out, 6, b)
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    const AP: [u8; 6] = [2, 0, 0, 0, 0, 1];
    const STA: [u8; 6] = [2, 0, 0, 0, 0, 2];
    fn fixture(message: u8, replay: u64) -> Vec<u8> {
        let mut b = vec![0, 0, 8, 0, 0, 0, 0, 0];
        let mut h = vec![0u8; 24];
        let ap = message == 1 || message == 3;
        h[0] = 8;
        h[1] = if ap { 2 } else { 1 };
        h[4..10].copy_from_slice(if ap { &STA } else { &AP });
        h[10..16].copy_from_slice(if ap { &AP } else { &STA });
        h[16..22].copy_from_slice(&AP);
        b.extend(h);
        b.extend([0xaa, 0xaa, 3, 0, 0, 0, 0x88, 0x8e]);
        let mut e = vec![0u8; 99];
        e[0] = 2;
        e[1] = 3;
        e[3] = 95;
        e[4] = 2;
        let info: u16 = match message {
            1 => 0x008a,
            2 => 0x010a,
            3 => 0x03ca,
            _ => 0x030a,
        };
        e[5..7].copy_from_slice(&info.to_be_bytes());
        e[9..17].copy_from_slice(&replay.to_be_bytes());
        e[17..49].fill(if ap { 0x11 } else { 0x22 });
        b.extend(e);
        b
    }
    #[test]
    fn parses_four_messages_without_keys() {
        for m in 1..=4 {
            let (p, k) = parse(&fixture(m, 1), 1., AP).unwrap();
            assert_eq!(p.message, Some(m));
            assert_eq!(k.unwrap().station, mac(&STA));
        }
    }
    #[test]
    fn unrelated_truncated_and_malformed_frames_rejected() {
        let data = fixture(1, 1);
        for end in 0..data.len() {
            assert!(parse(&data[..end], 1., AP).is_none())
        }
        assert!(parse(&data, 1., STA).is_none());
        let mut malformed = data;
        malformed[3] = 255;
        assert!(parse(&malformed, 1., AP).is_none());
    }
    #[test]
    fn correlates_out_of_order_and_retransmission() {
        let mut tracker = Tracker::default();
        for m in [3, 2, 1, 2, 4] {
            tracker.observe(
                parse(&fixture(m, if m < 3 { 7 } else { 8 }), 10., AP)
                    .unwrap()
                    .1
                    .unwrap(),
            )
        }
        assert!(tracker.exchanges()[0].complete);
        assert_eq!(tracker.exchanges()[0].messages.len(), 4);
    }
    #[test]
    fn mismatched_counter_nonce_or_station_never_completes() {
        let mut t = Tracker::default();
        for m in 1..=4 {
            let mut e = parse(&fixture(m, m as u64), 1., AP).unwrap().1.unwrap();
            if m == 3 {
                e.nonce = [33; 32]
            }
            t.observe(e)
        }
        assert!(!t.exchanges()[0].complete);
    }
    #[test]
    fn export_has_valid_block_lengths() {
        let bytes = pcapng(&[RawFrame {
            time: 1.,
            data: fixture(1, 1),
        }]);
        let mut i = 0;
        let mut blocks = 0;
        while i < bytes.len() {
            let len = u32::from_le_bytes(bytes[i + 4..i + 8].try_into().unwrap()) as usize;
            assert_eq!(len % 4, 0);
            assert_eq!(bytes[i + 4..i + 8], bytes[i + len - 4..i + len]);
            i += len;
            blocks += 1
        }
        assert_eq!(blocks, 3);
    }
}
