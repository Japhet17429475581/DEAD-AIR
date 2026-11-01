use crate::{
    model::*,
    packets::{self, RawFrame, Tracker},
};
use serde::Deserialize;
use std::{process::Stdio, sync::Arc, time::Duration};
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
#[derive(Default)]
struct Data {
    status: CaptureStatus,
    frames: Vec<RawFrame>,
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
        self.data.lock().await.status.clone()
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
        if (adapter.connected && !request.allow_disconnect) || !adapter.monitor_supported {
            return Err(
                "Monitor support is required. To use a connected card, explicitly allow temporary disconnection.".into(),
            );
        }
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
                            Some("ready")=>{shared.lock().await.status.status="WAITING FOR CLIENT AUTHENTICATION".into()},
                            Some("done")=>{clean=true;break},
                            Some("error")=>{failure=Some(message["message"].as_str().unwrap_or("Helper failed").to_string());break},
                            Some("packet")=>{
                                let bytes:Vec<u8>=match serde_json::from_value(message["data"].clone()){Ok(v)=>v,Err(_)=>continue};if bytes.len()>4096{continue}
                                let time=message["time"].as_f64().unwrap_or(0.);if !time.is_finite()||time<=0.{continue}
                                if let Some((packet,key))=packets::parse(&bytes,time,bssid){
                                    let observations=crate::pmkid::observe(&bytes,time,bssid);
                                    if let Some(key)=key{tracker.observe(key)}
                                    let mut data=shared.lock().await;
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
            if !diagnostic.trim().is_empty() && failure.is_none() {
                failure = Some(diagnostic.chars().take(1000).collect());
            }
            let mut data = shared.lock().await;
            data.status.running = false;
            data.status.error = failure;
            data.status.status = if data.status.error.is_some() {
                "FAILED / CHECK RESTORATION"
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
