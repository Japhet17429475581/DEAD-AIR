#![cfg(target_os = "linux")]
use dead_air::{
    lab::{LabRequest, LabService},
    model::Policy,
    sessions::Sessions,
};

async fn completed(lab: &mut LabService) -> dead_air::model::LabStatus {
    for _ in 0..180 {
        let status = lab.status().await;
        if !status.running {
            return status;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    lab.stop().await.unwrap();
    panic!("Real Hashcat job exceeded test deadline");
}

#[tokio::test]
#[ignore = "requires installed Hashcat and an actual OpenCL runtime"]
async fn known_answer_is_recovered_and_private_inputs_removed() {
    let temp = tempfile::tempdir().unwrap();
    let mut sessions = Sessions::new(temp.path().join("sessions"));
    sessions.start(Policy::Temporary).unwrap();
    let directory = sessions.lab_dir().unwrap();
    let mut lab = LabService::default();
    lab.start(
        LabRequest {
            kind: "known-answer".into(),
            authorized: true,
            password: Some("DEADAIR-fixture-2003".into()),
            wordlist: Some("wrong-answer\nDEADAIR-fixture-2003\n".into()),
        },
        directory.clone(),
    )
    .await
    .unwrap();
    let status = completed(&mut lab).await;
    assert_eq!(
        status.exit_code,
        Some(0),
        "{}: {:?}",
        status.status,
        status.lines
    );
    assert_eq!(status.status, "KNOWN VECTOR RECOVERED");
    assert!(!status.lines.join("\n").contains("DEADAIR-fixture-2003"));
    assert!(!directory.join("candidates.txt").exists());
    assert!(!directory.join("vector.sha256").exists());
    sessions.end().unwrap();
    assert!(!directory.exists());
}

#[tokio::test]
#[ignore = "requires installed Hashcat and an actual OpenCL runtime"]
async fn benchmark_reports_speed_and_cancellation_reaps_process() {
    let temp = tempfile::tempdir().unwrap();
    let mut sessions = Sessions::new(temp.path().join("sessions"));
    sessions.start(Policy::Temporary).unwrap();
    let mut lab = LabService::default();
    lab.start(
        LabRequest {
            kind: "benchmark".into(),
            authorized: true,
            password: None,
            wordlist: None,
        },
        sessions.lab_dir().unwrap(),
    )
    .await
    .unwrap();
    let status = completed(&mut lab).await;
    assert_eq!(
        status.exit_code,
        Some(0),
        "{}: {:?}",
        status.status,
        status.lines
    );
    assert!(
        status.lines.iter().any(|s| s.starts_with("Speed.#")),
        "{}: {:?}",
        status.status,
        status.lines
    );
    lab.start(
        LabRequest {
            kind: "benchmark".into(),
            authorized: true,
            password: None,
            wordlist: None,
        },
        sessions.lab_dir().unwrap(),
    )
    .await
    .unwrap();
    let stopped = lab.stop().await.unwrap();
    assert!(!stopped.running);
    assert_eq!(stopped.status, "STOPPED");
    sessions.end().unwrap();
}
