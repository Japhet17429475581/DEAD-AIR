//! Evidence-based receive diagnostics. Timers never manufacture packet events.
use serde::{Deserialize, Serialize};

pub const CAPTURE_LIMIT_SECONDS: u64 = 300;
pub const NO_FRAMES_WARNING_SECONDS: u64 = 15;
pub const STALE_REPORT_SECONDS: u64 = 10;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectorReport {
    pub records_read: u64,
    pub pcap_bytes: Option<u64>,
    pub actual_frequency: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureProgress {
    pub state: String,
    pub elapsed_seconds: u64,
    pub remaining_seconds: u64,
    pub limit_seconds: u64,
    pub records_read: Option<u64>,
    pub pcap_bytes: Option<u64>,
    pub beacons: u64,
    pub eapol_frames: u64,
    pub actual_frequency: Option<u32>,
    pub heartbeat_age_seconds: Option<u64>,
    pub helper_version: Option<String>,
    pub note: String,
}

pub fn progress(
    elapsed: u64,
    heartbeat_age: Option<u64>,
    report: Option<&CollectorReport>,
    target_frames: u64,
    beacons: u64,
    eapol_frames: u64,
    helper_version: Option<&str>,
) -> CaptureProgress {
    let (state, note) = if heartbeat_age.is_some_and(|age| age >= STALE_REPORT_SECONDS) {
        ("HELPER_REPORTS_STALE", "Capture health reports stopped arriving. The last counters are retained; current receiver health is unknown. Stop and inspect helper/restoration diagnostics.")
    } else if report.is_none() && elapsed >= STALE_REPORT_SECONDS {
        ("NO_HELPER_TELEMETRY", "No receive-health report from the installed helper. Update both the application and system helper; the absence of this report does not prove a radio failure.")
    } else if report.is_some_and(|r| r.pcap_bytes.is_none()) && elapsed >= 5 {
        ("CAPTURE_FILE_NOT_READY", "The collector has not produced a readable capture file. Reception is not confirmed. Inspect collector diagnostics; this is not a wait for client authentication.")
    } else if report
        .is_some_and(|r| r.pcap_bytes.is_some_and(|bytes| bytes > 24) && r.records_read == 0)
        && elapsed >= NO_FRAMES_WARNING_SECONDS
    {
        ("PCAP_RECORDS_NOT_READ", "The capture file has data after its header, but no complete PCAP records have been read. Inspect the collector/file-reader path rather than assuming that no client is connected.")
    } else if target_frames == 0 && elapsed >= NO_FRAMES_WARNING_SECONDS {
        if report.is_some_and(|r| r.records_read > 0) {
            ("NO_MATCHING_TARGET_FRAMES", "The capture file contains records, but none were accepted for this exact BSSID. Check target/BSSID and frequency; a client reconnect will not fix a mismatched target or unreadable frames.")
        } else {
            ("NO_TARGET_FRAMES", "No frames from the selected access point have been received. This is unconfirmed reception, not a wait for a client. Check actual frequency, signal, rfkill and the driver. Handshake activity cannot be assessed yet.")
        }
    } else if eapol_frames > 0 {
        ("EAPOL_OBSERVED", "Real EAPOL-Key frames were observed. An exchange can remain partial; sequence presence does not cryptographically verify it.")
    } else if target_frames > 0 {
        ("TARGET_OBSERVED", "Frames from the selected access point were received; no EAPOL-Key frames were observed yet. An already connected client need not repeat authentication. Reconnect your consenting test client normally to check a fresh exchange.")
    } else {
        ("RECEPTION_UNCONFIRMED", "Collector started; reception of the selected access point is not confirmed yet. Only received packets will increase frame counters.")
    };
    CaptureProgress {
        state: state.into(),
        elapsed_seconds: elapsed,
        remaining_seconds: CAPTURE_LIMIT_SECONDS.saturating_sub(elapsed),
        limit_seconds: CAPTURE_LIMIT_SECONDS,
        records_read: report.map(|r| r.records_read),
        pcap_bytes: report.and_then(|r| r.pcap_bytes),
        beacons,
        eapol_frames,
        actual_frequency: report.and_then(|r| r.actual_frequency),
        heartbeat_age_seconds: heartbeat_age,
        helper_version: helper_version.map(str::to_owned),
        note: note.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn silent_radio_is_not_presented_as_a_healthy_wait_for_clients() {
        let report = CollectorReport {
            records_read: 0,
            pcap_bytes: Some(24),
            actual_frequency: Some(2437),
        };
        assert_eq!(
            progress(2, Some(0), Some(&report), 0, 0, 0, Some("test")).state,
            "RECEPTION_UNCONFIRMED"
        );
        let p = progress(20, Some(1), Some(&report), 0, 0, 0, Some("test"));
        assert_eq!(p.state, "NO_TARGET_FRAMES");
        assert_eq!(p.records_read, Some(0));
        assert_eq!(p.beacons, 0);
        assert_eq!(p.remaining_seconds, 280);
    }
    #[test]
    fn unrelated_records_and_target_reception_are_different_evidence() {
        let report = CollectorReport {
            records_read: 8,
            pcap_bytes: Some(1000),
            ..Default::default()
        };
        assert_eq!(
            progress(20, Some(0), Some(&report), 0, 0, 0, None).state,
            "NO_MATCHING_TARGET_FRAMES"
        );
        assert_eq!(
            progress(20, Some(0), Some(&report), 8, 8, 0, None).state,
            "TARGET_OBSERVED"
        );
        assert_eq!(
            progress(20, Some(0), Some(&report), 8, 6, 2, None).state,
            "EAPOL_OBSERVED"
        );
    }
    #[test]
    fn stale_and_missing_helper_reports_are_not_claimed_as_live_health() {
        let report = CollectorReport::default();
        assert_eq!(
            progress(20, None, None, 0, 0, 0, None).state,
            "NO_HELPER_TELEMETRY"
        );
        assert_eq!(
            progress(20, Some(12), Some(&report), 30, 30, 0, None).state,
            "HELPER_REPORTS_STALE"
        );
        assert_eq!(
            progress(310, Some(0), Some(&report), 0, 0, 0, None).remaining_seconds,
            0
        );
    }
    #[test]
    fn missing_output_and_unread_records_are_distinct_from_a_silent_radio() {
        let mut report = CollectorReport::default();
        assert_eq!(
            progress(20, Some(0), Some(&report), 0, 0, 0, None).state,
            "CAPTURE_FILE_NOT_READY"
        );
        report.pcap_bytes = Some(1000);
        assert_eq!(
            progress(20, Some(0), Some(&report), 0, 0, 0, None).state,
            "PCAP_RECORDS_NOT_READ"
        );
    }
}
