use crate::{
    capture::{CaptureRequest, CaptureService},
    lab::{LabRequest, LabService},
    model::*,
    sessions::Sessions,
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Instant,
};
use tauri::State;
use tokio::sync::Mutex;

pub struct Backend {
    session: Arc<Mutex<Sessions>>,
    mac: Mutex<crate::mac::MacService>,
    panic_active: AtomicBool,
    capture: Mutex<CaptureService>,
    lab: Mutex<LabService>,
    operation: Mutex<()>,
    scan_generation: Arc<AtomicU64>,
    last_scan: Mutex<Option<Instant>>,
    latest: Mutex<Vec<Network>>,
}
impl Backend {
    pub fn new(root: PathBuf) -> Self {
        Self {
            session: Arc::new(Mutex::new(Sessions::new(root))),
            mac: Mutex::new(crate::mac::MacService::default()),
            panic_active: AtomicBool::new(false),
            capture: Mutex::new(CaptureService::default()),
            lab: Mutex::new(LabService::default()),
            operation: Mutex::new(()),
            scan_generation: Arc::new(AtomicU64::new(0)),
            last_scan: Mutex::new(None),
            latest: Mutex::new(vec![]),
        }
    }
    pub async fn close(&self) -> Result<(), String> {
        if self.panic_active.load(Ordering::SeqCst) {
            return Err("PANIC cleanup is pending; use its retry action before closing".into());
        }
        let _op = self.operation.lock().await;
        self.scan_generation.fetch_add(1, Ordering::SeqCst);
        let status = self.capture.lock().await.stop().await?;
        self.lab.lock().await.stop().await?;
        self.mac.lock().await.stop().await?;
        if let Some(error) = status.error {
            if error.contains("restoration")
                || error.contains("Cleanup")
                || error.contains("journal")
            {
                return Err(error);
            }
        }
        let mut session = self.session.lock().await;
        if session.current.is_some() {
            session.end()?;
        }
        self.capture.lock().await.clear().await?;
        self.lab.lock().await.clear().await?;
        self.latest.lock().await.clear();
        crate::privilege::shutdown().await?;
        Ok(())
    }
    fn ready(&self) -> Result<(), String> {
        if self.panic_active.load(Ordering::SeqCst) {
            Err("PANIC cleanup pending; new operations are disabled".into())
        } else {
            Ok(())
        }
    }
}
#[tauri::command]
pub async fn system_check() -> Result<Diagnostics, String> {
    tokio::time::timeout(
        std::time::Duration::from_secs(45),
        crate::diagnostics::check(),
    )
    .await
    .map_err(|_| "System dependency check timed out".into())
}
#[tauri::command]
pub async fn discover(state: State<'_, Backend>, adapter: String) -> Result<Snapshot, String> {
    let _op = state.operation.lock().await;
    state.ready()?;
    state.session.lock().await.active()?;
    if state.capture.lock().await.status().await.running {
        return Err("Stop observation before discovery".into());
    }
    {
        let mut last = state.last_scan.lock().await;
        if last.is_some_and(|t| t.elapsed().as_secs() < 10) {
            return Err("Discovery throttled; wait 10 seconds between requests".into());
        }
        *last = Some(Instant::now());
    }
    let result = crate::network::discover(&adapter, state.scan_generation.clone()).await?;
    *state.latest.lock().await = result.networks.clone();
    Ok(result)
}
#[tauri::command]
pub async fn cancel_discovery(state: State<'_, Backend>) -> Result<(), String> {
    state.scan_generation.fetch_add(1, Ordering::SeqCst);
    Ok(())
}
#[tauri::command]
pub async fn session_start(state: State<'_, Backend>, policy: Policy) -> Result<Session, String> {
    let _op = state.operation.lock().await;
    state.ready()?;
    state.session.lock().await.start(policy)
}
#[tauri::command]
pub async fn session_status(state: State<'_, Backend>) -> Result<Session, String> {
    state.session.lock().await.status()
}
#[tauri::command]
pub async fn session_end(state: State<'_, Backend>) -> Result<Session, String> {
    state.close().await?;
    state.session.lock().await.status()
}
#[tauri::command]
pub async fn session_recover(state: State<'_, Backend>) -> Result<Vec<Session>, String> {
    state.session.lock().await.recover()
}
#[tauri::command]
pub async fn session_purge(state: State<'_, Backend>, id: String) -> Result<(), String> {
    let _op = state.operation.lock().await;
    state.ready()?;
    state.session.lock().await.purge(&id)
}
#[tauri::command]
pub async fn report_export(state: State<'_, Backend>, report: String) -> Result<String, String> {
    let _op = state.operation.lock().await;
    state.ready()?;
    if report.len() > 1024 * 1024 {
        return Err("Report size exceeded".into());
    }
    let _: serde_json::Value = serde_json::from_str(&report).map_err(|_| "Invalid JSON report")?;
    state
        .session
        .lock()
        .await
        .write("report", "json", report.as_bytes())
}
#[tauri::command]
pub async fn capture_start(
    state: State<'_, Backend>,
    request: CaptureRequest,
) -> Result<CaptureStatus, String> {
    let _op = state.operation.lock().await;
    state.ready()?;
    state.session.lock().await.active()?;
    if !state
        .latest
        .lock()
        .await
        .iter()
        .any(|n| n.bssid.eq_ignore_ascii_case(&request.bssid) && n.frequency == request.frequency)
    {
        return Err("Select an exact BSSID/frequency from a current live discovery result".into());
    }
    state.capture.lock().await.start(request).await
}
#[tauri::command]
pub async fn capture_status(state: State<'_, Backend>) -> Result<CaptureStatus, String> {
    Ok(state.capture.lock().await.status().await)
}
#[tauri::command]
pub async fn capture_stop(state: State<'_, Backend>) -> Result<CaptureStatus, String> {
    state.capture.lock().await.stop().await
}
#[tauri::command]
pub async fn capture_export(state: State<'_, Backend>) -> Result<String, String> {
    let _op = state.operation.lock().await;
    state.ready()?;
    let bytes = state.capture.lock().await.export().await?;
    state
        .session
        .lock()
        .await
        .write("capture", "pcapng", &bytes)
}
#[tauri::command]
pub async fn hashcat_devices() -> Result<String, String> {
    crate::process::output("hashcat", &["-I"], 25).await
}
#[tauri::command]
pub async fn lab_start(
    state: State<'_, Backend>,
    request: LabRequest,
) -> Result<LabStatus, String> {
    let _op = state.operation.lock().await;
    state.ready()?;
    let path = state.session.lock().await.lab_dir()?;
    state.lab.lock().await.start(request, path).await
}
#[tauri::command]
pub async fn lab_status(state: State<'_, Backend>) -> Result<LabStatus, String> {
    Ok(state.lab.lock().await.status().await)
}
#[tauri::command]
pub async fn lab_stop(state: State<'_, Backend>) -> Result<LabStatus, String> {
    state.lab.lock().await.stop().await
}
#[tauri::command]
pub async fn kismet_check(token: String) -> Result<String, String> {
    crate::diagnostics::kismet_check(token).await
}
#[tauri::command]
pub async fn dependency_plan(profile: String) -> Result<crate::setup::SetupPlan, String> {
    crate::setup::plan(&profile)
}
#[tauri::command]
pub async fn adapter_report() -> Result<String, String> {
    crate::privilege::output(&["adapter-report"], 25).await
}
#[tauri::command]
pub async fn recover_capture(state: State<'_, Backend>) -> Result<String, String> {
    let _op = state.operation.lock().await;
    if state.capture.lock().await.status().await.running {
        return Err("Stop the current capture first".into());
    }
    let result = crate::privilege::output(&["recover-owned"], 30).await?;
    state.capture.lock().await.acknowledge_recovery().await;
    Ok(result)
}
#[tauri::command]
pub async fn install_dependencies(
    state: State<'_, Backend>,
    profile: String,
    authorized: bool,
) -> Result<crate::setup::SetupPlan, String> {
    if !authorized {
        return Err("Confirm the package installation shown in the setup screen".into());
    }
    let _op = state.operation.lock().await;
    state.ready()?;
    if state.capture.lock().await.status().await.running
        || state.lab.lock().await.status().await.running
    {
        return Err("Stop jobs before installing dependencies".into());
    }
    crate::setup::plan(&profile)?;
    crate::privilege::output(&["install-dependencies", &profile], 2450).await?;
    crate::setup::plan(&profile)
}

#[tauri::command]
pub async fn privileged_status() -> bool {
    crate::privilege::active().await
}
#[tauri::command]
pub async fn privileged_authorize(state: State<'_, Backend>) -> Result<bool, String> {
    state.ready()?;
    let _command = crate::privilege::command(&["adapter-report"]).await?;
    Ok(crate::privilege::active().await)
}
#[tauri::command]
pub async fn mac_status(state: State<'_, Backend>) -> Result<crate::mac::MacStatus, String> {
    Ok(state.mac.lock().await.status())
}
#[tauri::command]
pub async fn mac_start(
    state: State<'_, Backend>,
    adapter: String,
    profile: String,
) -> Result<crate::mac::MacStatus, String> {
    let _op = state.operation.lock().await;
    state.ready()?;
    if state.capture.lock().await.status().await.running {
        return Err("Stop capture before changing MAC".into());
    }
    state.mac.lock().await.start(&adapter, &profile).await
}
#[tauri::command]
pub async fn mac_restore(state: State<'_, Backend>) -> Result<crate::mac::MacStatus, String> {
    let _op = state.operation.lock().await;
    if state.capture.lock().await.status().await.running {
        return Err("Stop capture before restoring MAC".into());
    }
    state.mac.lock().await.stop().await
}
#[tauri::command]
pub async fn mac_recover(state: State<'_, Backend>) -> Result<String, String> {
    let _op = state.operation.lock().await;
    if state.mac.lock().await.status().active || state.capture.lock().await.status().await.running {
        return Err("Stop active MAC/capture operations before recovery".into());
    }
    let result = crate::privilege::output(&["mac-recover"], 45).await?;
    state.mac.lock().await.recovered();
    Ok(result)
}
#[tauri::command]
pub async fn panic_cleanup(
    state: State<'_, Backend>,
    app: tauri::AppHandle,
    all_sessions: bool,
    shred: bool,
) -> Result<usize, String> {
    state.panic_active.store(true, Ordering::SeqCst);
    state.scan_generation.fetch_add(1, Ordering::SeqCst);
    let _op=tokio::time::timeout(std::time::Duration::from_secs(30),state.operation.lock()).await.map_err(|_|"An operation is still finishing (possibly a system installation). PANIC did not delete data; retry when it finishes.")?;
    let capture = state.capture.lock().await.stop().await;
    let lab = state.lab.lock().await.stop().await;
    let mac = state.mac.lock().await.stop().await;
    state.latest.lock().await.clear();
    if let Ok(status) = &capture {
        if let Some(error) = &status.error {
            return Err(format!("Capture restoration requires inspection: {error}"));
        }
    }
    capture?;
    lab?;
    mac?;
    state.capture.lock().await.clear().await?;
    state.lab.lock().await.clear().await?;
    let sessions = state.session.clone();
    let count = tokio::task::spawn_blocking(move || {
        sessions.blocking_lock().panic_purge(all_sessions, shred)
    })
    .await
    .map_err(|e| e.to_string())??;
    crate::privilege::shutdown().await?;
    app.exit(0);
    Ok(count)
}

#[tauri::command]
pub async fn open_reference(url: String) -> Result<(), String> {
    let parsed = reqwest::Url::parse(&url).map_err(|_| "Invalid reference URL")?;
    let hosts = [
        "csrc.nist.gov",
        "wireless.docs.kernel.org",
        "networkmanager.dev",
        "www.networkmanager.dev",
        "www.aircrack-ng.org",
        "wiki.wireshark.org",
        "hashcat.net",
        "arubanetworking.hpe.com",
        "www.freedesktop.org",
        "www.kismetwireless.net",
        "github.com",
        "www.gnu.org",
    ];
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
        || !parsed.host_str().is_some_and(|h| hosts.contains(&h))
    {
        return Err("Only the curriculum's HTTPS documentation hosts can be opened".into());
    }
    let mut child = crate::process::command("xdg-open")?
        .arg(parsed.as_str())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    tokio::spawn(async move {
        let _ = child.wait().await;
    });
    Ok(())
}
