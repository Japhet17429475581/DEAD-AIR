#![cfg(target_os = "linux")]
//! Synthetic offline fixtures processed by the actual distribution airodump-ng.
//! This validates file compatibility, not monitor-mode reception.
use dead_air::packets::{parse, read_pcap_record};
use std::{fs::File, io::Write, process::Stdio};
const AP: [u8; 6] = [2, 0xda, 0, 0, 1, 1];
const STA: [u8; 6] = [2, 0xda, 0, 0, 2, 2];
fn header(first: u8, flags: u8, receiver: [u8; 6], sender: [u8; 6]) -> Vec<u8> {
    let mut frame = vec![0; 24];
    frame[0] = first;
    frame[1] = flags;
    frame[4..10].copy_from_slice(&receiver);
    frame[10..16].copy_from_slice(&sender);
    frame[16..22].copy_from_slice(&AP);
    frame
}
fn beacon() -> Vec<u8> {
    let mut frame = header(0x80, 0, [0xff; 6], AP);
    frame.extend([0; 8]);
    frame.extend([100, 0, 0x11, 0x04]);
    frame.extend([0, 7]);
    frame.extend(b"FIXTURE");
    frame.extend([1, 4, 0x82, 0x84, 0x8b, 0x96, 3, 1, 6]);
    frame
}
fn eapol(message: u8) -> Vec<u8> {
    let ap = message == 1 || message == 3;
    let mut frame = header(
        8,
        if ap { 2 } else { 1 },
        if ap { STA } else { AP },
        if ap { AP } else { STA },
    );
    frame.extend([0xaa, 0xaa, 3, 0, 0, 0, 0x88, 0x8e]);
    let mut key = vec![0; 99];
    key[0] = 2;
    key[1] = 3;
    key[3] = 95;
    key[4] = 2;
    let info: u16 = match message {
        1 => 0x008a,
        2 => 0x010a,
        3 => 0x03ca,
        _ => 0x030a,
    };
    key[5..7].copy_from_slice(&info.to_be_bytes());
    key[7..9].copy_from_slice(&16u16.to_be_bytes());
    key[9..17].copy_from_slice(&(if message < 3 { 7u64 } else { 8u64 }).to_be_bytes());
    key[17..49].fill(if ap { 0x11 } else { 0x22 });
    frame.extend(key);
    frame
}
#[tokio::test]
#[ignore = "requires installed airodump-ng; offline synthetic PCAP only, no radio I/O"]
async fn actual_airodump_file_delivers_beacons_and_eapol_to_the_application_parser() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("fixture.pcap");
    let mut file = File::create(&input).unwrap();
    let mut global = vec![0xd4, 0xc3, 0xb2, 0xa1, 2, 0, 4, 0];
    global.extend([0; 8]);
    global.extend(65535u32.to_le_bytes());
    global.extend(105u32.to_le_bytes());
    file.write_all(&global).unwrap();
    for (i, frame) in [beacon(), beacon(), eapol(1), eapol(2), eapol(3), eapol(4)]
        .iter()
        .enumerate()
    {
        for n in [
            1_700_000_000 + i as u32,
            0,
            frame.len() as u32,
            frame.len() as u32,
        ] {
            file.write_all(&n.to_le_bytes()).unwrap();
        }
        file.write_all(frame).unwrap();
    }
    drop(file);
    let mut command = dead_air::process::command("airodump-ng").unwrap();
    command
        .args([
            "--background",
            "1",
            "--beacons",
            "--bssid",
            "02:DA:00:00:01:01",
            "--output-format",
            "pcap",
            "--write-interval",
            "1",
            "-r",
        ])
        .arg(&input)
        .arg("--write")
        .arg(dir.path().join("capture"))
        .env("TERM", "dumb")
        .stdin(Stdio::null());
    // airodump keeps its display/event loop alive at offline EOF. Stop it
    // normally once its output contains all fixture records, so buffers flush.
    command.stdout(Stdio::null()).stderr(Stdio::null());
    let mut child = command.spawn().unwrap();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(8);
    loop {
        let size = std::fs::metadata(dir.path().join("capture-01.cap"))
            .map(|m| m.len())
            .unwrap_or(0);
        if size > 500 {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            let _ = child.kill().await;
            panic!("Offline airodump produced no complete fixture output");
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    unsafe {
        libc::kill(child.id().unwrap() as i32, libc::SIGINT);
    }
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(4), child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
    let mut output = File::open(dir.path().join("capture-01.cap")).unwrap();
    let mut offset = 0;
    let mut beacons = 0;
    let mut messages = vec![];
    while let Some(frame) = read_pcap_record(&mut output, &mut offset).unwrap() {
        if let Some((packet, _)) = parse(&frame.data, frame.time, AP) {
            if packet.kind == "Beacon" {
                beacons += 1;
            }
            if let Some(message) = packet.message {
                messages.push(message);
            }
        }
    }
    assert_eq!(
        beacons, 2,
        "All-beacon recording must preserve evidence of reception"
    );
    assert_eq!(
        messages,
        [1, 2, 3, 4],
        "Actual airodump output must remain readable by the app"
    );
}
