use std::{process::Stdio, sync::LazyLock, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::Mutex,
};
struct Daemon {
    child: Child,
    input: Option<ChildStdin>,
    path: String,
    token: String,
}
static DAEMON: LazyLock<Mutex<Option<Daemon>>> = LazyLock::new(|| Mutex::new(None));
pub async fn active() -> bool {
    let mut daemon = DAEMON.lock().await;
    if let Some(d) = daemon.as_mut() {
        return matches!(d.child.try_wait(), Ok(None));
    }
    false
}
pub async fn command(args: &[&str]) -> Result<Command, String> {
    let mut slot = DAEMON.lock().await;
    if let Some(d) = slot.as_mut() {
        if !matches!(d.child.try_wait(), Ok(None)) {
            *slot = None;
        }
    }
    if slot.is_none() {
        let token = uuid::Uuid::new_v4().to_string();
        let mut child = crate::process::command("pkexec")?
            .args([crate::capture::HELPER, "broker", &token])
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        let input = child.stdin.take();
        let mut reader = BufReader::new(child.stdout.take().ok_or("Broker stdout missing")?);
        let mut bytes = Vec::new();
        let result = tokio::time::timeout(
            Duration::from_secs(120),
            (&mut reader).take(4096).read_until(b'\n', &mut bytes),
        )
        .await;
        if !matches!(result,Ok(Ok(n)) if n>0&&n<4096) {
            let _ = child.kill().await;
            return Err(
                "Privileged daemon authorization failed or timed out; no password was stored"
                    .into(),
            );
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if value["kind"] != "broker-ready" {
            return Err(format!(
                "Cannot start privileged daemon: {}",
                value["message"].as_str().unwrap_or("unexpected reply")
            ));
        }
        *slot = Some(Daemon {
            child,
            input,
            path: value["path"].as_str().ok_or("Broker path missing")?.into(),
            token,
        });
    }
    let daemon = slot.as_ref().unwrap();
    let mut command = Command::new(crate::capture::HELPER);
    command
        .args(["broker-client", &daemon.path, &daemon.token])
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    Ok(command)
}
pub async fn output(args: &[&str], seconds: u64) -> Result<String, String> {
    crate::process::output_command(command(args).await?, "privileged helper", seconds).await
}
pub async fn shutdown() -> Result<(), String> {
    let mut slot = DAEMON.lock().await;
    if let Some(d) = slot.as_mut() {
        d.input.take();
        match tokio::time::timeout(Duration::from_secs(15), d.child.wait()).await {
            Ok(Ok(status)) if status.success() => (),
            _ => {
                return Err(
                    "Privileged daemon has not finished cleanup; retry after operations stop"
                        .into(),
                )
            }
        }
    }
    *slot = None;
    Ok(())
}
