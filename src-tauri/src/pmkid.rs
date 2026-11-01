//! Passive PMKID observation. No frame transmission or password recovery.
use serde::Serialize;
#[derive(Clone, Serialize)]
pub struct Observation {
    pub station: String,
    pub bssid: String,
    pub value: String,
    pub source: String,
    pub time: f64,
}
fn rsn_ids(bytes: &[u8]) -> Option<Vec<[u8; 16]>> {
    fn take<'a>(data: &mut &'a [u8], n: usize) -> Option<&'a [u8]> {
        let value = data.get(..n)?;
        *data = &data[n..];
        Some(value)
    }
    fn count(data: &mut &[u8]) -> Option<usize> {
        let n = take(data, 2)?;
        Some(u16::from_le_bytes([n[0], n[1]]) as usize)
    }
    let mut data = bytes;
    if take(&mut data, 2)? != [1, 0] {
        return None;
    }
    take(&mut data, 4)?;
    let pairs = count(&mut data)?;
    if pairs > 64 {
        return None;
    }
    take(&mut data, pairs.checked_mul(4)?)?;
    let akms = count(&mut data)?;
    if akms > 64 {
        return None;
    }
    take(&mut data, akms.checked_mul(4)?)?;
    if data.is_empty() {
        return Some(vec![]);
    }
    take(&mut data, 2)?;
    if data.is_empty() {
        return Some(vec![]);
    }
    let n = count(&mut data)?;
    if n > 16 {
        return None;
    }
    let list = take(&mut data, n.checked_mul(16)?)?;
    Some(
        list.chunks_exact(16)
            .filter(|v| v.iter().any(|b| *b != 0))
            .map(|v| v.try_into().unwrap())
            .collect(),
    )
}
pub fn information_elements(mut data: &[u8], kde: bool) -> Vec<[u8; 16]> {
    let mut result = Vec::new();
    let mut count = 0;
    while data.len() >= 2 && count < 128 {
        count += 1;
        let tag = data[0];
        let n = data[1] as usize;
        let Some(value) = data.get(2..2 + n) else {
            break;
        };
        if tag == 48 {
            if let Some(ids) = rsn_ids(value) {
                result.extend(ids);
            }
        }
        if kde
            && tag == 221
            && value.len() == 20
            && value[..4] == [0, 0x0f, 0xac, 4]
            && value[4..].iter().any(|v| *v != 0)
        {
            result.push(value[4..].try_into().unwrap());
        }
        data = &data[2 + n..];
    }
    result.truncate(16);
    result
}
pub fn observe(bytes: &[u8], time: f64, bssid: [u8; 6]) -> Vec<Observation> {
    let Some((offset, _)) = crate::packets::radiotap(bytes) else {
        return vec![];
    };
    let Some(frame) = bytes.get(offset..) else {
        return vec![];
    };
    if frame.len() < 24 {
        return vec![];
    }
    let fc = u16::from_le_bytes([frame[0], frame[1]]);
    if fc & 3 != 0 || fc & 0x4400 != 0 || frame[22] & 15 != 0 {
        return vec![];
    }
    let kind = (fc >> 2) & 3;
    let subtype = (fc >> 4) & 15;
    let a1 = &frame[4..10];
    let a2 = &frame[10..16];
    let a3 = &frame[16..22];
    let (station, ids, source) =
        if kind == 0 && matches!(subtype, 0 | 2) && a1 == bssid && a3 == bssid {
            let fixed = if subtype == 0 { 4 } else { 10 };
            let Some(ies) = frame.get(24 + fixed..) else {
                return vec![];
            };
            (a2, information_elements(ies, false), "RSN association list")
        } else if kind == 2 && fc & 0x200 != 0 && fc & 0x100 == 0 && a2 == bssid {
            if crate::packets::parse(bytes, time, bssid).is_none() {
                return vec![];
            }
            let mut header = 24;
            if subtype & 8 != 0 {
                header += 2;
                if fc & 0x8000 != 0 {
                    header += 4;
                }
            }
            let Some(e) = frame.get(header + 8..) else {
                return vec![];
            };
            if e.len() < 99 {
                return vec![];
            }
            let info = u16::from_be_bytes([e[5], e[6]]);
            if info & 0x1000 != 0 {
                return vec![];
            }
            let n = u16::from_be_bytes([e[97], e[98]]) as usize;
            let declared = u16::from_be_bytes([e[2], e[3]]) as usize + 4;
            if 99 + n > declared {
                return vec![];
            }
            let Some(data) = e.get(99..99 + n) else {
                return vec![];
            };
            (a1, information_elements(data, true), "EAPOL key data")
        } else {
            return vec![];
        };
    if station[0] & 1 != 0 {
        return vec![];
    }
    ids.into_iter()
        .map(|id| Observation {
            station: crate::packets::mac(station),
            bssid: crate::packets::mac(&bssid),
            value: id.iter().map(|b| format!("{b:02x}")).collect(),
            source: source.into(),
            time,
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn rsn() -> Vec<u8> {
        let mut v = vec![
            1, 0, 0, 0x0f, 0xac, 4, 1, 0, 0, 0x0f, 0xac, 4, 1, 0, 0, 0x0f, 0xac, 2, 0, 0, 1, 0,
        ];
        v.extend([0x11; 16]);
        v
    }
    #[test]
    fn parses_rsn_and_vendor_kde_but_rejects_truncated_counts() {
        let r = rsn();
        let mut ie = vec![48, r.len() as u8];
        ie.extend(r.clone());
        assert_eq!(information_elements(&ie, false), vec![[0x11; 16]]);
        for end in 0..ie.len() {
            assert!(information_elements(&ie[..end], false).is_empty());
        }
        let mut bad = r;
        bad[20] = 255;
        let mut ie = vec![48, bad.len() as u8];
        ie.extend(bad);
        assert!(information_elements(&ie, false).is_empty());
        let mut kde = vec![221, 20, 0, 0x0f, 0xac, 4];
        kde.extend([0x22; 16]);
        assert_eq!(information_elements(&kde, true), vec![[0x22; 16]]);
        assert!(information_elements(&kde, false).is_empty());
    }
    #[test]
    fn association_observation_is_bound_to_ap_and_station() {
        let ap = [2, 0, 0, 0, 0, 1];
        let sta = [2, 0, 0, 0, 0, 2];
        let mut frame = vec![0; 28];
        frame[4..10].copy_from_slice(&ap);
        frame[10..16].copy_from_slice(&sta);
        frame[16..22].copy_from_slice(&ap);
        let r = rsn();
        frame.extend([48, r.len() as u8]);
        frame.extend(r);
        let mut bytes = vec![0, 0, 8, 0, 0, 0, 0, 0];
        bytes.extend(frame);
        let found = observe(&bytes, 1., ap);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].station, "02:00:00:00:00:02");
        assert!(observe(&bytes, 1., sta).is_empty());
    }
    #[test]
    fn observes_plain_eapol_kde_but_not_encrypted_or_incomplete_key_data() {
        let ap = [2, 0, 0, 0, 0, 1];
        let sta = [2, 0, 0, 0, 0, 2];
        let mut frame = vec![0; 24];
        frame[0] = 8;
        frame[1] = 2;
        frame[4..10].copy_from_slice(&sta);
        frame[10..16].copy_from_slice(&ap);
        frame[16..22].copy_from_slice(&ap);
        frame.extend([0xaa, 0xaa, 3, 0, 0, 0, 0x88, 0x8e]);
        let mut e = vec![0; 99];
        e[0] = 2;
        e[1] = 3;
        e[2..4].copy_from_slice(&117u16.to_be_bytes());
        e[4] = 2;
        e[5..7].copy_from_slice(&0x008au16.to_be_bytes());
        e[9..17].copy_from_slice(&1u64.to_be_bytes());
        e[17..49].fill(0x11);
        e[97..99].copy_from_slice(&22u16.to_be_bytes());
        e.extend([221, 20, 0, 0x0f, 0xac, 4]);
        e.extend([0x33; 16]);
        frame.extend(e);
        let mut bytes = vec![0, 0, 8, 0, 0, 0, 0, 0];
        bytes.extend(frame);
        assert_eq!(observe(&bytes, 1., ap).len(), 1);
        for end in 0..bytes.len() {
            assert!(observe(&bytes[..end], 1., ap).is_empty());
        }
        let mut encrypted = bytes.clone();
        encrypted[8 + 24 + 8 + 5] |= 0x10;
        assert!(observe(&encrypted, 1., ap).is_empty());
        let mut fragmented = bytes;
        fragmented[8 + 22] = 1;
        assert!(observe(&fragmented, 1., ap).is_empty());
    }
}
