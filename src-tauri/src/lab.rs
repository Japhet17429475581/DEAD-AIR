use crate::model::LabStatus;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader},
    sync::{oneshot, Mutex},
    task::JoinHandle,
};
use zeroize::Zeroizing;

#[derive(Deserialize)]
pub struct LabRequest {
    pub kind: String,
    pub authorized: bool,
    pub password: Option<String>,
    pub wordlist: Option<String>,
}
pub struct LabService {
    data: Arc<Mutex<LabStatus>>,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}
impl Default for LabService {
    fn default() -> Self {
        Self {
            data: Arc::new(Mutex::new(LabStatus::default())),
            stop: None,
            task: None,
        }
    }
}
pub fn safe_line(line: &str) -> Option<String> {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
        if v.get("status").is_some() {
            return Some(serde_json::json!({"status":v["status"],"progress":v["progress"],"recovered_hashes":v["recovered_hashes"],"devices":v["devices"].as_array().map(|devices|devices.iter().map(|d|serde_json::json!({"device_id":d["device_id"],"speed":d["speed"],"temp":d["temp"],"util":d["util"]})).collect::<Vec<_>>())}).to_string());
        }
    }
    let line = line.trim();
    let safe = [
        "Speed.#",
        "Status.",
        "Progress.",
        "Recovered.",
        "Hardware.Mon.",
        "hashcat (",
        "No devices found",
        "No OpenCL",
        "clGetPlatformIDs",
        "CUDA SDK Toolkit",
        "Started:",
        "Stopped:",
    ];
    safe.iter().any(|prefix| line.starts_with(prefix)).then(|| {
        line.chars()
            .filter(|c| !c.is_control())
            .take(1000)
            .collect()
    })
}
async fn read_lines<R: AsyncRead + Unpin>(stream: R, data: Arc<Mutex<LabStatus>>) {
    let mut reader = BufReader::new(stream);
    loop {
        let mut bytes = Vec::new();
        let result = (&mut reader)
            .take(16384)
            .read_until(b'\n', &mut bytes)
            .await;
        match result {
            Ok(0) | Err(_) => break,
            _ => (),
        };
        if bytes.len() >= 16384 {
            continue;
        }
        if let Some(line) = safe_line(&String::from_utf8_lossy(&bytes)) {
            let mut state = data.lock().await;
            if state.lines.len() >= 200 {
                state.lines.remove(0);
            }
            state.lines.push(line)
        }
    }
}
impl LabService {
    pub async fn status(&self) -> LabStatus {
        self.data.lock().await.clone()
    }
    pub async fn start(
        &mut self,
        mut request: LabRequest,
        directory: PathBuf,
    ) -> Result<LabStatus, String> {
        if !request.authorized {
            return Err("Confirm compute-job authorization".into());
        }
        if self.status().await.running {
            return Err("A Hashcat job is already running".into());
        }
        if !matches!(request.kind.as_str(), "benchmark" | "known-answer") {
            return Err("Only benchmarks and generated known-answer vectors are supported".into());
        }
        let help = crate::process::output("hashcat", &["--help"], 10).await?;
        for flag in [
            "--potfile-disable",
            "--restore-disable",
            "--logfile-disable",
            "--runtime",
        ] {
            if !help.contains(flag) {
                return Err(format!("Installed Hashcat lacks required option {flag}"));
            }
        }
        let mut command = crate::process::command("hashcat")?;
        command
            .current_dir(&directory)
            .env("HOME", &directory)
            .env("XDG_CACHE_HOME", directory.join("cache"))
            .env("XDG_DATA_HOME", directory.join("data"))
            .env("TMPDIR", &directory)
            .env("CUDA_CACHE_PATH", directory.join("cuda-cache"))
            .env("POCL_CACHE_DIR", directory.join("pocl-cache"));
        command.args([
            "--potfile-disable",
            "--restore-disable",
            "--logfile-disable",
            "--runtime",
            "300",
            "-m",
            "1400",
            "--opencl-device-types",
            "1,2",
            "--session",
            "deadair-known-lab",
        ]);
        if help.contains("--cache-path") {
            command.arg("--cache-path").arg(&directory);
        }
        let mut input_paths = Vec::new();
        if request.kind == "benchmark" {
            command.arg("--benchmark");
        } else {
            let password = Zeroizing::new(
                request
                    .password
                    .take()
                    .ok_or("Provide a known test password")?,
            );
            let wordlist = Zeroizing::new(
                request
                    .wordlist
                    .take()
                    .ok_or("Provide a local lab wordlist")?,
            );
            if password.is_empty()
                || password.len() > 512
                || wordlist.is_empty()
                || wordlist.len() > 4 * 1024 * 1024
            {
                return Err("Test vector or wordlist exceeds the lab bounds".into());
            }
            if wordlist.contains('\0') {
                return Err("Wordlist must be text without NUL bytes".into());
            }
            let hash = format!("{:x}\n", Sha256::digest(password.as_bytes()));
            #[cfg(target_os = "linux")]
            {
                let dir = crate::storage::Dir::root(&directory)?;
                dir.write("vector.sha256", hash.as_bytes())?;
                dir.write("candidates.txt", wordlist.as_bytes())?;
            }
            #[cfg(not(target_os = "linux"))]
            return Err("Hashcat jobs require Linux".into());
            input_paths.extend([
                directory.join("vector.sha256"),
                directory.join("candidates.txt"),
            ]);
            command.args([
                "-a",
                "0",
                "--status",
                "--status-timer",
                "1",
                "--outfile",
                "/dev/null",
            ]);
            if help.contains("--status-json") {
                command.arg("--status-json");
            }
            command.arg(&input_paths[0]).arg(&input_paths[1]);
        }
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(e) => {
                for path in &input_paths {
                    let _ = std::fs::remove_file(path);
                }
                return Err(e.to_string());
            }
        };
        let stdout = child.stdout.take().ok_or("Hashcat stdout unavailable")?;
        let stderr = child.stderr.take().ok_or("Hashcat stderr unavailable")?;
        *self.data.lock().await=LabStatus{running:true,status:"RUNNING".into(),lines:vec!["Real Hashcat process started. Output is restricted to status and device statistics.".into()],exit_code:None};
        let out_task = tokio::spawn(read_lines(stdout, self.data.clone()));
        let err_task = tokio::spawn(read_lines(stderr, self.data.clone()));
        let (tx, rx) = oneshot::channel();
        self.stop = Some(tx);
        let shared = self.data.clone();
        let benchmark = request.kind == "benchmark";
        self.task = Some(tokio::spawn(async move {
            let mut stopped = false;
            let mut timed_out = false;
            let result = tokio::select! {r=child.wait()=>r,_=rx=>{stopped=true;let _=child.kill().await;child.wait().await},_=tokio::time::sleep(Duration::from_secs(330))=>{timed_out=true;let _=child.kill().await;child.wait().await}};
            let _ = out_task.await;
            let _ = err_task.await;
            for path in &input_paths {
                let _ = std::fs::remove_file(path);
            }
            let mut state = shared.lock().await;
            state.running = false;
            state.exit_code = result.ok().and_then(|s| s.code());
            state.status = if stopped {
                "STOPPED"
            } else if timed_out {
                "TIMED OUT"
            } else {
                match state.exit_code {
                    Some(0) => {
                        if benchmark {
                            "BENCHMARK COMPLETED"
                        } else {
                            "KNOWN VECTOR RECOVERED"
                        }
                    }
                    Some(1) => "WORDLIST EXHAUSTED",
                    Some(2) => "ABORTED",
                    Some(3) => "CHECKPOINT ABORT",
                    Some(4) => "RUNTIME LIMIT",
                    _ => "FAILED / CHECK HASHCAT DEVICES AND RUNTIME",
                }
            }
            .into();
        }));
        Ok(self.status().await)
    }
    pub async fn stop(&mut self) -> Result<LabStatus, String> {
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(());
        }
        if let Some(task) = self.task.take() {
            task.await.map_err(|e| e.to_string())?
        }
        Ok(self.status().await)
    }
    pub async fn clear(&mut self) -> Result<(), String> {
        self.stop().await?;
        *self.data.lock().await = LabStatus::default();
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn never_exposes_candidates_or_plaintexts() {
        assert!(safe_line("Candidates.#1....: secret -> password").is_none());
        assert!(safe_line("0123456789abcdef:my-secret").is_none());
        assert!(safe_line("Hash.Target......: private-hash").is_none());
        assert!(safe_line("Speed.#1.........: 123 H/s").is_some());
    }
    #[test]
    fn json_is_reconstructed_from_allowlisted_fields() {
        let line=safe_line(r#"{"status":2,"progress":[1,10],"password":"secret","guess":{"base":"secret"},"devices":[{"device_id":1,"speed":42,"candidate":"secret"}]}"#).unwrap();
        assert!(!line.contains("secret"));
        assert!(line.contains("42"));
    }
}
