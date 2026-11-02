#![cfg(target_os = "linux")]
use dead_air::aircrack::linux::{remove_runtime, Airodump};
use std::time::{Duration, Instant};

#[test]
#[ignore = "requires installed airodump-ng, root and writable /dev/shm tmpfs; uses a nonexistent radio"]
fn actual_airodump_failure_reaches_the_application_without_discarding_stderr() {
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let root = tempfile::Builder::new()
        .prefix("deadair-diagnostic-test-")
        .tempdir_in("/dev/shm")
        .unwrap();
    let directory = root.path().join("capture");
    let mut collector = Airodump::start(
        directory.clone(),
        "danonexistent",
        "02:da:00:00:01:01",
        2437,
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let error = loop {
        match collector.next() {
            Err(error) => break error,
            Ok(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            Ok(_) => panic!("airodump did not report the missing radio"),
        }
    };
    assert!(error.contains("airodump-ng exited unexpectedly"), "{error}");
    assert!(!error.contains("No stderr output"), "{error}");
    assert!(
        error.contains("danonexistent") || error.contains("No such device"),
        "{error}"
    );
    drop(collector);
    remove_runtime(&directory).unwrap();
    assert!(!directory.exists());
}
