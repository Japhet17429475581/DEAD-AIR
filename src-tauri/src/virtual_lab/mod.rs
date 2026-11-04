//! Native virtual WLAN labs. Real Linux protocol stacks, isolated hwsim radios,
//! no physical interface input and no fallback to demonstration results.
use serde::{Deserialize, Serialize};
use std::{process::Stdio, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    sync::Mutex,
    task::JoinHandle,
};
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
mod netlink;

pub const LAB_ID: &str = "wpa2-basics";
pub const LAB_SSID: &str = "DEADAIR-LAB";
pub const LAB_PASSPHRASE: &str = "LabOnly-2003!";
pub const LIMIT_SECONDS: u64 = 600;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualStatus {
    pub running: bool,
    pub busy: bool,
    pub phase: String,
    pub operation_id: String,
    pub ssid: String,
    pub bssid: String,
    pub ap_ready: bool,
    pub scan_confirmed: bool,
    pub scan_ssids: Vec<String>,
    #[serde(default)]
    pub client_started: bool,
    pub client_connected: bool,
    pub ap_authorized: bool,
    pub ping_verified: bool,
    pub eapol_messages: Vec<u8>,
    pub exchange_correlated: bool,
    pub completed: bool,
    pub packet_count: u64,
    pub elapsed_seconds: u64,
    pub limit_seconds: u64,
    pub logs: Vec<String>,
    pub error: Option<String>,
    pub cleanup_complete: bool,
    #[serde(default)]
    pub packets: Vec<crate::model::Packet>,
}
impl VirtualStatus {
    pub fn idle() -> Self {
        Self {
            phase: "NOT STARTED".into(),
            ssid: LAB_SSID.into(),
            limit_seconds: LIMIT_SECONDS,
            cleanup_complete: true,
            ..Default::default()
        }
    }
    pub fn has_completion_evidence(&self) -> bool {
        self.ap_ready
            && self.scan_confirmed
            && self.client_connected
            && self.ap_authorized
            && self.ping_verified
            && self.exchange_correlated
            && [1, 2, 3, 4].iter().all(|m| self.eapol_messages.contains(m))
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub supported: bool,
    pub platform: String,
    pub tools: Vec<crate::model::ToolStatus>,
    pub kernel_module: bool,
    pub module_loaded: bool,
    pub details: Vec<String>,
}
pub async fn capabilities() -> Capabilities {
    let mut result = Capabilities {
        supported: false,
        platform: std::env::consts::OS.into(),
        tools: vec![],
        kernel_module: false,
        module_loaded: false,
        details: vec![],
    };
    if !cfg!(target_os = "linux") {
        result.details.push("Real virtual WLAN labs require Linux with mac80211_hwsim. No simulated fallback is used.".into());
        return result;
    }
    for name in [
        "ip",
        "iw",
        "modprobe",
        "hostapd",
        "wpa_supplicant",
        "airodump-ng",
        "ping",
    ] {
        let available = crate::process::executable(name).is_ok();
        result.tools.push(crate::model::ToolStatus {
            name: name.into(),
            available,
            version: if available { "Installed" } else { "Missing" }.into(),
            detail: "System program used by the isolated lab".into(),
        });
    }
    result.module_loaded = std::path::Path::new("/sys/module/mac80211_hwsim").is_dir();
    result.kernel_module = result.module_loaded
        || crate::process::output("modinfo", &["mac80211_hwsim"], 5)
            .await
            .is_ok();
    if !result.kernel_module {
        result.details.push("The running kernel does not expose an available mac80211_hwsim module. A matching kernel/module package is required; see the Linux lab guide.".into());
    }
    result.details.push("Start requires polkit authorization and network-namespace capabilities. Containers with restricted capabilities can fail even when tools are installed.".into());
    result.supported = result.kernel_module && result.tools.iter().all(|tool| tool.available);
    result
}

#[derive(Default)]
struct Data {
    status: VirtualStatus,
    frames: Vec<crate::packets::RawFrame>,
}
pub struct VirtualLabService {
    data: Arc<Mutex<Data>>,
    input: Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    task: Option<JoinHandle<()>>,
}
impl Default for VirtualLabService {
    fn default() -> Self {
        Self {
            data: Arc::new(Mutex::new(Data {
                status: VirtualStatus::idle(),
                frames: vec![],
            })),
            input: Arc::new(Mutex::new(None)),
            task: None,
        }
    }
}
impl VirtualLabService {
    pub async fn status(&self) -> VirtualStatus {
        self.data.lock().await.status.clone()
    }
    pub async fn start(&mut self, lab_id: &str, authorized: bool) -> Result<VirtualStatus, String> {
        if lab_id != LAB_ID {
            return Err("Unknown built-in virtual lab".into());
        }
        if !authorized {
            return Err("Confirm creation of this isolated kernel-backed lab".into());
        }
        let before = self.status().await;
        if before.running {
            return Err("A virtual lab is already running".into());
        }
        if !before.cleanup_complete {
            return Err("Recover the previous virtual lab before starting again".into());
        }
        if !cfg!(target_os = "linux") {
            return Err("Real virtual WLAN labs require Linux".into());
        }
        if let Some(task) = self.task.take() {
            task.await.map_err(|e| e.to_string())?;
        }
        let token = uuid::Uuid::new_v4().to_string();
        let mut child = crate::privilege::command(&["virtual-lab", LAB_ID, &token])
            .await?
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        *self.input.lock().await = child.stdin.take();
        let stdout = child.stdout.take().ok_or("Virtual lab stdout missing")?;
        let stderr = child.stderr.take().ok_or("Virtual lab stderr missing")?;
        let mut diagnostic = tokio::spawn(async move {
            let mut bytes = vec![];
            let _ = stderr.take(8192).read_to_end(&mut bytes).await;
            String::from_utf8_lossy(&bytes).into_owned()
        });
        *self.data.lock().await = Data {
            status: VirtualStatus {
                running: true,
                busy: true,
                phase: "CREATING ISOLATED LAB".into(),
                operation_id: token.clone(),
                ssid: LAB_SSID.into(),
                limit_seconds: LIMIT_SECONDS,
                ..Default::default()
            },
            frames: vec![],
        };
        let data = self.data.clone();
        let input = self.input.clone();
        self.task = Some(tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut failure = None;
            let mut clean = false;
            let deadline = tokio::time::sleep(Duration::from_secs(LIMIT_SECONDS + 120));
            tokio::pin!(deadline);
            loop {
                let mut bytes = vec![];
                let mut limited = (&mut reader).take(24576);
                let result = tokio::select! {_=&mut deadline=>{failure=Some("Virtual lab lifetime exceeded; inspect cleanup".into());break;},r=limited.read_until(b'\n',&mut bytes)=>r};
                if !matches!(result,Ok(n)if n>0&&n<24576) {
                    if !matches!(result, Ok(0)) {
                        failure = Some("Virtual lab protocol closed or exceeded bounds".into());
                    }
                    break;
                }
                let value: serde_json::Value = match serde_json::from_slice(&bytes) {
                    Ok(value) => value,
                    Err(_) => {
                        failure = Some("Malformed virtual lab protocol".into());
                        break;
                    }
                };
                match value["kind"].as_str() {
                    Some("lab-state") => {
                        let mut status = match serde_json::from_value::<VirtualStatus>(
                            value["status"].clone(),
                        ) {
                            Ok(s) => s,
                            Err(e) => {
                                failure = Some(format!("Invalid virtual lab state: {e}"));
                                break;
                            }
                        };
                        if status.operation_id != token
                            || status.packets.len() > 40
                            || status.logs.len() > 80
                        {
                            failure = Some("Virtual lab state identity or bounds mismatch".into());
                            break;
                        }
                        status.completed = status.completed && status.has_completion_evidence();
                        let mut shared = data.lock().await;
                        status.packets = std::mem::take(&mut shared.status.packets);
                        shared.status = status;
                    }
                    Some("packet") => {
                        let raw = match serde_json::from_value::<Vec<u8>>(value["data"].clone()) {
                            Ok(raw) if raw.len() <= 4096 => raw,
                            _ => continue,
                        };
                        let time = value["time"].as_f64().unwrap_or(0.);
                        if !time.is_finite() || time <= 0. {
                            continue;
                        }
                        let mut shared = data.lock().await;
                        if let Ok(bssid) = crate::model::parse_mac(&shared.status.bssid) {
                            if let Some((packet, _)) = crate::packets::parse(&raw, time, bssid) {
                                if shared.status.packets.len() >= 40 {
                                    shared.status.packets.remove(0);
                                }
                                shared.status.packets.push(packet);
                                if shared.frames.len() < 4096 {
                                    shared
                                        .frames
                                        .push(crate::packets::RawFrame { time, data: raw });
                                }
                            }
                        }
                    }
                    Some("lab-done") => {
                        clean = value["clean"].as_bool() == Some(true);
                        break;
                    }
                    Some("error") => {
                        failure = Some(
                            value["message"]
                                .as_str()
                                .unwrap_or("Virtual lab failed")
                                .into(),
                        );
                        break;
                    }
                    _ => {
                        failure = Some("Unknown virtual lab protocol message".into());
                        break;
                    }
                }
            }
            input.lock().await.take();
            let exited = matches!(tokio::time::timeout(Duration::from_secs(45),child.wait()).await,Ok(Ok(code))if code.success());
            if !exited {
                let _ = child.kill().await;
                failure.get_or_insert("Virtual lab helper exit was not confirmed".into());
            }
            match tokio::time::timeout(Duration::from_secs(2), &mut diagnostic).await {
                Ok(Ok(text)) if !text.trim().is_empty() => {
                    let message = failure.get_or_insert(String::new());
                    message.push_str(&text);
                }
                Err(_) => {
                    diagnostic.abort();
                    failure.get_or_insert("Virtual lab diagnostics remained open".into());
                }
                _ => {}
            }
            if !clean && failure.is_none() {
                failure = Some("Virtual lab stopped without cleanup acknowledgement".into());
            }
            let mut shared = data.lock().await;
            shared.status.running = false;
            shared.status.busy = false;
            shared.status.cleanup_complete = clean && exited && failure.is_none();
            if let Some(error) = failure {
                shared.status.error = Some(error);
                shared.status.phase = "FAILED / CHECK LAB RECOVERY".into();
            } else {
                shared.status.phase = "STOPPED / OWNED RESOURCES REMOVED".into();
                shared.status.error = None;
            }
        }));
        Ok(self.status().await)
    }
    pub async fn action(&self, action: &str) -> Result<VirtualStatus, String> {
        if !matches!(action, "scan" | "connect" | "verify") {
            return Err("Unknown virtual lab action".into());
        }
        {
            let mut data = self.data.lock().await;
            if !data.status.running || data.status.busy {
                return Err("Wait for the active virtual lab operation".into());
            }
            data.status.busy = true;
            data.status.error = None;
        }
        let result = async {
            let mut input = self.input.lock().await;
            let writer = input.as_mut().ok_or("Virtual lab input is closed")?;
            writer
                .write_all(format!("{action}\n").as_bytes())
                .await
                .map_err(|e| e.to_string())
        }
        .await;
        if let Err(error) = result {
            self.data.lock().await.status.busy = false;
            return Err(error);
        }
        Ok(self.status().await)
    }
    pub async fn stop(&mut self) -> Result<VirtualStatus, String> {
        self.input.lock().await.take();
        if let Some(task) = self.task.take() {
            task.await.map_err(|e| e.to_string())?;
        }
        let status = self.status().await;
        if !status.cleanup_complete {
            return Err(status
                .error
                .unwrap_or("Virtual lab cleanup requires recovery".into()));
        }
        Ok(status)
    }
    pub async fn recovered(&self) {
        let mut data = self.data.lock().await;
        if !data.status.running {
            data.status.cleanup_complete = true;
            data.status.error = None;
            data.status.phase = "RECOVERY VERIFIED".into();
        }
    }
    pub async fn export(&self) -> Result<Vec<u8>, String> {
        let data = self.data.lock().await;
        if data.status.running {
            return Err("Stop the virtual lab before exporting".into());
        }
        if data.frames.is_empty() {
            return Err("No virtual lab frames retained".into());
        }
        Ok(crate::packets::pcapng(&data.frames))
    }
    pub async fn clear(&mut self) -> Result<(), String> {
        self.stop().await?;
        *self.data.lock().await = Data {
            status: VirtualStatus::idle(),
            frames: vec![],
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completion_requires_all_actual_evidence() {
        let mut status = VirtualStatus {
            ap_ready: true,
            scan_confirmed: true,
            client_connected: true,
            ap_authorized: true,
            ping_verified: true,
            exchange_correlated: false,
            ..Default::default()
        };
        assert!(!status.has_completion_evidence());
        status.exchange_correlated = true;
        assert!(!status.has_completion_evidence());
        status.eapol_messages = vec![1, 2, 3];
        assert!(!status.has_completion_evidence());
        status.eapol_messages.push(4);
        assert!(status.has_completion_evidence());
        status.ping_verified = false;
        assert!(!status.has_completion_evidence());
    }
}
