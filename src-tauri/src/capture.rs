use crate::{
    model::*,
    packets::{self, RawFrame, Tracker},
};
use serde::Deserialize;
use std::{
    process::Stdio,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    sync::{oneshot, Mutex},
    task::JoinHandle,
};

pub const HELPER: &str = "/usr/libexec/deadair-capture-helper";
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRequest {
    pub adapter: String,
    pub bssid: String,
    pub frequency: u32,
    pub authorized: bool,
    #[serde(default)]
    pub allow_disconnect: bool,
}
fn validate_capture_adapter(adapter: &Adapter, allow_disconnect: bool) -> Result<(), String> {
    if adapter.connected && !allow_disconnect {
        return Err("Allow temporary Wi-Fi disconnection before using this connected card".into());
    }
    if !matches!(adapter.mode.as_str(), "managed" | "monitor") {
        return Err(format!(
            "Capture requires a managed or monitor interface, not {}",
            adapter.mode
        ));
    }
    if adapter.mode != "monitor" && !adapter.monitor_supported {
        return Err(format!("{} / {}: iw did not report monitor support. Refresh adapters and check the driver capabilities.", adapter.name, adapter.driver));
    }
    Ok(())
}
#[derive(Default)]
struct Data {
    status: CaptureStatus,
    frames: Vec<RawFrame>,
    ready_at: Option<Instant>,
    finished_at: Option<Instant>,
    last_report_at: Option<Instant>,
    report: Option<crate::capture_health::CollectorReport>,
    helper_version: Option<String>,
    beacons: u64,
    eapol_frames: u64,
    restoring: bool,
}
impl Data {
    fn snapshot(&self, now: Instant) -> CaptureStatus {
        let mut status = self.status.clone();
        if let Some(started) = self.ready_at {
            let end = self.finished_at.unwrap_or(now);
            let report_age = self
                .last_report_at
                .map(|last| end.saturating_duration_since(last).as_secs());
            let mut progress = crate::capture_health::progress(
                end.saturating_duration_since(started).as_secs(),
                if self.restoring || !status.running {
                    report_age.map(|_| 0)
                } else {
                    report_age
                },
                self.report.as_ref(),
                status.count,
                self.beacons,
                self.eapol_frames,
                self.helper_version.as_deref(),
            );
            progress.heartbeat_age_seconds = report_age;
            if status.running && self.restoring {
                progress.state = "RESTORING".into();
                progress.note="The collector is stopping and the adapter is being restored. Receive-health reports are not expected during restoration; no further client authentication is being awaited.".into();
            }
            if status.running && self.restoring {
                status.status = "STOPPING / RESTORING INTERFACE".into();
            } else if status.running && status.exchanges.iter().all(|e| !e.complete) {
                status.status = if status.exchanges.is_empty() {
                    progress.state.clone()
                } else {
                    "EXCHANGE PARTIAL".into()
                };
            }
            status.progress = Some(progress);
        }
        status
    }
}
pub struct CaptureService {
    data: Arc<Mutex<Data>>,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}
impl Default for CaptureService {
    fn default() -> Self {
        Self {
            data: Arc::new(Mutex::new(Data::default())),
            stop: None,
            task: None,
        }
    }
}
impl CaptureService {
    pub async fn status(&self) -> CaptureStatus {
        self.data.lock().await.snapshot(Instant::now())
    }
    pub async fn start(&mut self, request: CaptureRequest) -> Result<CaptureStatus, String> {
        if !request.authorized {
            return Err("Ownership and participant consent must be confirmed".into());
        }
        if self.status().await.running {
            return Err("Capture already running".into());
        }
        if !valid_interface(&request.adapter) {
            return Err("Invalid capture interface".into());
        }
        let bssid = parse_mac(&request.bssid)?;
        let adapter = crate::network::adapters()
            .await?
            .into_iter()
            .find(|a| a.name == request.adapter)
            .ok_or("Capture interface no longer present")?;
        validate_capture_adapter(&adapter, request.allow_disconnect)?;
        if !std::path::Path::new(HELPER).is_file() {
            return Err("Install the scoped capture helper with scripts/install-helper.sh".into());
        }
        let token = uuid::Uuid::new_v4().to_string();
        let mut child = crate::privilege::command(&[
            "capture",
            &request.adapter,
            &request.bssid,
            &request.frequency.to_string(),
            &token,
            if request.allow_disconnect {
                "exclusive"
            } else {
                "dedicated"
            },
        ])
        .await?
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
        let stdout = child.stdout.take().ok_or("Helper stdout unavailable")?;
        let stderr = child.stderr.take().ok_or("Helper stderr unavailable")?;
        let mut stderr_task = tokio::spawn(async move {
            let mut data = Vec::new();
            let _ = stderr.take(8192).read_to_end(&mut data).await;
            String::from_utf8_lossy(&data).into_owned()
        });
        let (tx, mut rx) = oneshot::channel();
        self.stop = Some(tx);
        {
            let mut data = self.data.lock().await;
            *data = Data::default();
            data.status.running = true;
            data.status.status = "AUTHORIZATION / PREFLIGHT".into();
        }
        let shared = self.data.clone();
        self.task = Some(tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut tracker = Tracker::default();
            let deadline = tokio::time::sleep(Duration::from_secs(330));
            tokio::pin!(deadline);
            let mut failure = None;
            let mut clean = false;
            let mut stopping = false;
            loop {
                let mut line = Vec::new();
                let mut limited_reader = (&mut reader).take(24576);
                tokio::select! {
                    _=&mut rx,if !stopping=>{child.stdin.take();stopping=true;deadline.as_mut().reset(tokio::time::Instant::now()+Duration::from_secs(45));continue},
                    _=&mut deadline=>{failure=Some("Capture deadline exceeded".into());child.stdin.take();break},
                    result=limited_reader.read_until(b'\n',&mut line)=>{
                        match result{Ok(0)=>break,Ok(_)=>(),Err(e)=>{failure=Some(e.to_string());break}}
                        if line.len()>=24576{failure=Some("Helper output exceeded protocol limit".into());break}
                        let message:serde_json::Value=match serde_json::from_slice(&line){Ok(v)=>v,Err(_)=>{failure=Some("Malformed helper message".into());break}};
                        match message["kind"].as_str(){
                            Some("ready")=>{
                                let mut data=shared.lock().await;
                                data.ready_at=Some(Instant::now());
                                data.helper_version=message["version"].as_str().map(|s|s.chars().take(64).collect());
                                data.status.status="RECEPTION_UNCONFIRMED".into();
                            },
                            Some("progress")=>{
                                let report=match serde_json::from_value::<crate::capture_health::CollectorReport>(message["report"].clone()) {
                                    Ok(report)=>report,
                                    Err(_)=>{failure=Some("Malformed helper receive-health report".into());break;}
                                };
                                let mut data=shared.lock().await;
                                data.last_report_at=Some(Instant::now());data.report=Some(report);
                            },
                            Some("restoring")=>{shared.lock().await.restoring=true;},
                            Some("done")=>{
                                shared.lock().await.status.stop_reason=message["reason"].as_str().map(|s|s.chars().take(64).collect());
                                clean=true;break
                            },
                            Some("error")=>{failure=Some(message["message"].as_str().unwrap_or("Helper failed").to_string());break},
                            Some("packet")=>{
                                let bytes:Vec<u8>=match serde_json::from_value(message["data"].clone()){Ok(v)=>v,Err(_)=>continue};if bytes.len()>4096{continue}
                                let time=message["time"].as_f64().unwrap_or(0.);if !time.is_finite()||time<=0.{continue}
                                if let Some((packet,key))=packets::parse(&bytes,time,bssid){
                                    let observations=crate::pmkid::observe(&bytes,time,bssid);
                                    if let Some(key)=key{tracker.observe(key)}
                                    let mut data=shared.lock().await;
                                    if packet.kind=="Beacon" {data.beacons+=1;}
                                    if packet.message.is_some() {data.eapol_frames+=1;}
                                    for found in observations {if data.status.pmkids.len()<128&&!data.status.pmkids.iter().any(|old|old.station==found.station&&old.value==found.value){data.status.pmkids.push(found);}}
                                    data.status.count+=1;if data.frames.len()<4096{data.frames.push(RawFrame{time,data:bytes})}else{data.status.dropped+=1}
                                    if data.status.packets.len()>=200{data.status.packets.remove(0);}data.status.packets.push(packet);data.status.exchanges=tracker.exchanges();
                                    data.status.status=if data.status.exchanges.iter().any(|e|e.complete){"EXCHANGE OBSERVED (NOT CRYPTOGRAPHICALLY VERIFIED)"}else if !data.status.exchanges.is_empty(){"EXCHANGE PARTIAL"}else{"PASSIVE MONITORING"}.into();
                                }
                            },_=>{failure=Some("Unknown helper protocol message".into());break}
                        }
                    }
                }
            }
            child.stdin.take();
            let exit = match tokio::time::timeout(Duration::from_secs(45), child.wait()).await {
                Ok(Ok(status)) => Some(status),
                _ => {
                    let _ = child.kill().await;
                    failure.get_or_insert(
                        "Helper did not exit promptly. Inspect /run/deadair recovery journal."
                            .into(),
                    );
                    None
                }
            };
            let diagnostic =
                match tokio::time::timeout(Duration::from_secs(2), &mut stderr_task).await {
                    Ok(result) => result.unwrap_or_default(),
                    Err(_) => {
                        stderr_task.abort();
                        failure.get_or_insert(
                            "Helper output remained open. Inspect /run/deadair recovery journal."
                                .into(),
                        );
                        String::new()
                    }
                };
            if exit.is_some_and(|e| !e.success()) && failure.is_none() {
                failure = Some(format!(
                    "Capture helper failed. {}",
                    diagnostic.chars().take(1000).collect::<String>()
                ));
            }
            if !diagnostic.trim().is_empty() {
                let detail: String = diagnostic.chars().take(1000).collect();
                if let Some(message) = &mut failure {
                    if !message.contains(&detail) {
                        message.push('\n');
                        message.push_str(&detail);
                    }
                } else {
                    failure = Some(detail);
                }
            }
            let mut data = shared.lock().await;
            data.finished_at = Some(Instant::now());
            data.status.running = false;
            data.status.error = failure;
            data.status.status = if data.status.error.is_some() {
                "FAILED / CHECK RESTORATION"
            } else if clean && data.status.count == 0 {
                "STOPPED / NO TARGET FRAMES"
            } else if clean && data.status.exchanges.is_empty() {
                "STOPPED / NO EAPOL EXCHANGE"
            } else if clean && !data.status.exchanges.iter().any(|e| e.complete) {
                "STOPPED / PARTIAL EAPOL EXCHANGE"
            } else if clean {
                "STOPPED / RESOURCES RELEASED"
            } else {
                "STOPPED"
            }
            .into();
        }));
        Ok(self.status().await)
    }
    pub async fn stop(&mut self) -> Result<CaptureStatus, String> {
        if self.status().await.running {
            self.data.lock().await.restoring = true;
        }
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(());
        }
        if let Some(task) = self.task.take() {
            task.await.map_err(|e| e.to_string())?
        }
        Ok(self.status().await)
    }
    pub async fn export(&self) -> Result<Vec<u8>, String> {
        let data = self.data.lock().await;
        if data.status.running {
            return Err("Stop capture before export".into());
        }
        if data.frames.is_empty() {
            return Err("No retained frames".into());
        }
        Ok(packets::pcapng(&data.frames))
    }
    pub async fn clear(&mut self) -> Result<(), String> {
        self.stop().await?;
        *self.data.lock().await = Data::default();
        Ok(())
    }
    pub async fn acknowledge_recovery(&self) {
        let mut data = self.data.lock().await;
        if !data.status.running && data.status.error.is_some() {
            data.status.error = None;
            data.status.status = "STOPPED / RECOVERY CHECK COMPLETED".into();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_exposes_silence_without_creating_packets_and_freezes_after_stop() {
        let start = Instant::now();
        let mut data = Data::default();
        data.status.running = true;
        data.ready_at = Some(start);
        data.last_report_at = Some(start + Duration::from_secs(20));
        data.report = Some(crate::capture_health::CollectorReport {
            records_read: 0,
            pcap_bytes: Some(24),
            actual_frequency: Some(2437),
        });
        let status = data.snapshot(start + Duration::from_secs(21));
        assert_eq!(status.status, "NO_TARGET_FRAMES");
        assert_eq!(status.count, 0);
        assert!(status.packets.is_empty());
        assert!(status.exchanges.is_empty());
        data.restoring = true;
        assert_eq!(
            data.snapshot(start + Duration::from_secs(40))
                .progress
                .unwrap()
                .state,
            "RESTORING"
        );
        data.status.running = false;
        data.finished_at = Some(start + Duration::from_secs(30));
        assert_eq!(
            data.snapshot(start + Duration::from_secs(90))
                .progress
                .unwrap()
                .elapsed_seconds,
            30
        );
    }
    #[test]
    fn monitor_state_is_usable_without_advertised_capability_but_does_not_bypass_consent() {
        let mut adapter = Adapter {
            name: "wlan0mon".into(),
            driver: "fixture".into(),
            phy: "phy0".into(),
            mode: "monitor".into(),
            frequency: Some(2437),
            monitor_supported: false,
            connected: false,
            bands: vec![],
        };
        assert!(validate_capture_adapter(&adapter, false).is_ok());
        adapter.connected = true;
        assert!(validate_capture_adapter(&adapter, false).is_err());
        assert!(validate_capture_adapter(&adapter, true).is_ok());
        adapter.connected = false;
        adapter.mode = "managed".into();
        assert!(validate_capture_adapter(&adapter, false).is_err());
        adapter.monitor_supported = true;
        assert!(validate_capture_adapter(&adapter, false).is_ok());
        adapter.mode = "AP".into();
        assert!(validate_capture_adapter(&adapter, true).is_err());
    }
}
