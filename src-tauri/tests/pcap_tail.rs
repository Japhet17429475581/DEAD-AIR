use dead_air::packets::read_pcap_record;
use std::io::Write;

fn capture(little: bool, nano: bool, link: u32, size: u32) -> Vec<u8> {
    let mut bytes = vec![0u8; 24];
    bytes[..4].copy_from_slice(match (little, nano) {
        (true, false) => &[0xd4, 0xc3, 0xb2, 0xa1],
        (false, false) => &[0xa1, 0xb2, 0xc3, 0xd4],
        (true, true) => &[0x4d, 0x3c, 0xb2, 0xa1],
        (false, true) => &[0xa1, 0xb2, 0x3c, 0x4d],
    });
    let encode = |v: u32| {
        if little {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    };
    bytes[20..24].copy_from_slice(&encode(link));
    for number in [10, if nano { 500_000_000 } else { 500_000 }, size, size] {
        bytes.extend(encode(number));
    }
    bytes
}

#[test]
fn reads_both_byte_orders_and_timestamp_units() {
    for little in [false, true] {
        for nano in [false, true] {
            let mut file = tempfile::tempfile().unwrap();
            let mut bytes = capture(little, nano, 105, 3);
            bytes.extend([1, 2, 3]);
            file.write_all(&bytes).unwrap();
            let mut offset = 0;
            let frame = read_pcap_record(&mut file, &mut offset).unwrap().unwrap();
            assert_eq!(frame.time, 10.5);
            assert_eq!(frame.data, [0, 0, 8, 0, 0, 0, 0, 0, 1, 2, 3]);
            assert_eq!(offset, 43);
            assert!(read_pcap_record(&mut file, &mut offset).unwrap().is_none());
        }
    }
}

#[test]
fn incomplete_record_is_retried_without_consuming_it() {
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(&capture(true, false, 127, 3)).unwrap();
    file.write_all(&[1]).unwrap();
    let mut offset = 0;
    assert!(read_pcap_record(&mut file, &mut offset).unwrap().is_none());
    assert_eq!(offset, 24);
    use std::io::{Seek, SeekFrom};
    file.seek(SeekFrom::End(0)).unwrap();
    file.write_all(&[2, 3]).unwrap();
    assert_eq!(
        read_pcap_record(&mut file, &mut offset)
            .unwrap()
            .unwrap()
            .data,
        [1, 2, 3]
    );
}

#[test]
fn unsupported_links_and_oversized_records_fail_closed() {
    for (link, size) in [(1, 1), (127, 65536)] {
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(&capture(true, false, link, size)).unwrap();
        assert!(read_pcap_record(&mut file, &mut 0).is_err());
    }
}
