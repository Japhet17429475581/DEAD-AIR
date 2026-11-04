//! Per-application privileged broker. Fixed helper operations only; no shell,
//! arbitrary executable, package list, or filesystem destination is accepted.
#[cfg(target_os = "linux")]
pub mod linux {
    use serde::{Deserialize, Serialize};
    use std::{
        os::unix::fs::{MetadataExt, PermissionsExt},
        sync::Arc,
        time::Duration,
    };
    use tokio::{
        io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader},
        net::UnixStream,
        sync::{watch, Mutex},
        task::JoinSet,
    };
    #[derive(Deserialize, Serialize)]
    pub struct Request {
        pub token: String,
        pub args: Vec<String>,
    }
    #[derive(Deserialize, Serialize)]
    pub struct Frame {
        pub stream: String,
        pub data: Vec<u8>,
        pub code: Option<i32>,
    }
    pub fn allowed(args: &[String]) -> bool {
        if args.iter().any(|a| a.len() > 128 || a.contains('\0')) {
            return false;
        }
        match args.first().map(String::as_str) {
            Some("adapter-report" | "recover-owned" | "mac-recover") => args.len() == 1,
            Some("install-dependencies") => {
                args.len() == 2
                    && matches!(args[1].as_str(), "all" | "wireless" | "hashcat" | "cpu")
            }
            Some("capture") => {
                (args.len() == 5
                    || args.len() == 6 && matches!(args[5].as_str(), "exclusive" | "dedicated"))
                    && crate::model::valid_interface(&args[1])
                    && crate::model::parse_mac(&args[2]).is_ok()
                    && args[3].parse::<u32>().is_ok()
                    && uuid::Uuid::parse_str(&args[4]).is_ok()
            }
            Some("mac-profile") => {
                (args.len() == 4
                    || args.len() == 5 && matches!(args[4].as_str(), "dedicated" | "exclusive"))
                    && crate::model::valid_interface(&args[1])
                    && crate::mac::profile_mac(&args[2]).is_ok()
                    && uuid::Uuid::parse_str(&args[3]).is_ok()
            }
            _ => false,
        }
    }
    async fn line<R: AsyncRead + Unpin>(
        reader: &mut BufReader<R>,
        limit: u64,
    ) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        (&mut *reader)
            .take(limit)
            .read_until(b'\n', &mut bytes)
            .await
            .map_err(|e| e.to_string())?;
        if bytes.is_empty() || bytes.len() as u64 >= limit {
            return Err("Broker connection closed or message too large".into());
        }
        Ok(bytes)
    }
    async fn send<W: AsyncWrite + Unpin>(writer: &mut W, frame: &Frame) -> Result<(), String> {
        let mut bytes = serde_json::to_vec(frame).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        tokio::time::timeout(Duration::from_secs(5), writer.write_all(&bytes))
            .await
            .map_err(|_| "Broker receiver stalled")?
            .map_err(|e| e.to_string())
    }
    async fn forward<R: AsyncRead + Unpin>(
        mut input: R,
        writer: Arc<Mutex<tokio::net::unix::OwnedWriteHalf>>,
        stream: &str,
    ) {
        let mut bytes = [0; 4096];
        loop {
            match input.read(&mut bytes).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if send(
                        &mut *writer.lock().await,
                        &Frame {
                            stream: stream.into(),
                            data: bytes[..n].to_vec(),
                            code: None,
                        },
                    )
                    .await
                    .is_err()
                    {
                        break;
                    }
                }
            }
        }
    }
    async fn serve(
        socket: UnixStream,
        uid: u32,
        token: String,
        mut stopping: watch::Receiver<bool>,
    ) -> Result<(), String> {
        if socket.peer_cred().map_err(|e| e.to_string())?.uid() != uid {
            return Err("Broker UID mismatch".into());
        }
        let (read, write) = socket.into_split();
        let mut reader = BufReader::new(read);
        let bytes = tokio::time::timeout(Duration::from_secs(5), line(&mut reader, 4096))
            .await
            .map_err(|_| "Broker handshake timed out")??;
        let request: Request = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if request.token != token || !allowed(&request.args) || *stopping.borrow() {
            return Err("Broker request denied".into());
        }
        let mut command = tokio::process::Command::new(crate::capture::HELPER);
        command
            .args(&request.args)
            .env_clear()
            .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
            .env("LC_ALL", "C")
            .env("PKEXEC_UID", uid.to_string())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().map_err(|e| e.to_string())?;
        let mut stdin = child.stdin.take().ok_or("Missing child stdin")?;
        let writer = Arc::new(Mutex::new(write));
        let stdout = tokio::spawn(forward(
            child.stdout.take().unwrap(),
            writer.clone(),
            "stdout",
        ));
        let stderr = tokio::spawn(forward(
            child.stderr.take().unwrap(),
            writer.clone(),
            "stderr",
        ));
        let input = tokio::spawn(async move {
            loop {
                let bytes = tokio::select! {_=stopping.changed()=>break,result=line(&mut reader,32768)=>match result{Ok(b)=>b,Err(_)=>break}};
                let Ok(frame) = serde_json::from_slice::<Frame>(&bytes) else {
                    break;
                };
                if frame.stream == "eof" {
                    break;
                }
                if frame.stream != "stdin" || frame.data.len() > 4096 {
                    break;
                }
                if stdin.write_all(&frame.data).await.is_err() {
                    break;
                }
            }
            // EOF requests orderly capture/MAC restoration, not SIGKILL.
            drop(stdin);
        });
        // A MAC lease lasts until the client closes it; do not SIGKILL it merely
        // because the operator has kept the application open for a long time.
        let code = if request.args[0] == "mac-profile" {
            child
                .wait()
                .await
                .map_err(|e| e.to_string())?
                .code()
                .unwrap_or(1)
        } else {
            match tokio::time::timeout(Duration::from_secs(2700), child.wait()).await {
                Ok(Ok(status)) => status.code().unwrap_or(1),
                _ => {
                    let _ = child.kill().await;
                    1
                }
            }
        };
        input.abort();
        let _ = stdout.await;
        let _ = stderr.await;
        let result = send(
            &mut *writer.lock().await,
            &Frame {
                stream: "exit".into(),
                data: vec![],
                code: Some(code),
            },
        )
        .await;
        result
    }
    pub async fn daemon(uid: u32, token: String) -> Result<(), String> {
        uuid::Uuid::parse_str(&token).map_err(|_| "Invalid broker token")?;
        if unsafe { libc::geteuid() } != 0 {
            return Err("Broker requires polkit authorization".into());
        }
        let root = std::path::Path::new("/run/deadair-broker");
        if !root.exists() {
            std::fs::create_dir(root).map_err(|e| e.to_string())?;
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
        let meta = std::fs::symlink_metadata(root).map_err(|e| e.to_string())?;
        if !meta.is_dir() || meta.uid() != 0 || meta.mode() & 0o022 != 0 {
            return Err("Unsafe broker directory".into());
        }
        let path = root.join(format!("{uid}-{token}.sock"));
        // Restrict permissions at bind time, before any other process can connect.
        unsafe {
            libc::umask(0o077);
        }
        let listener = tokio::net::UnixListener::bind(&path).map_err(|e| e.to_string())?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
        let name = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
        if unsafe { libc::chown(name.as_ptr(), uid, u32::MAX) } != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let (stop_tx, stop_rx) = watch::channel(false);
        let lifetime = tokio::spawn(async move {
            let mut bytes = [0; 1];
            let _ = tokio::io::stdin().read(&mut bytes).await;
            let _ = stop_tx.send(true);
        });
        println!(
            "{}",
            serde_json::json!({"kind":"broker-ready","path":path,"pid":std::process::id()})
        );
        let mut jobs = JoinSet::new();
        let mut stopped = stop_rx.clone();
        loop {
            tokio::select! {
                _=stopped.changed()=>break,
                Some(_)=jobs.join_next(),if !jobs.is_empty()=>(),
                accepted=listener.accept()=>{let(socket,_)=accepted.map_err(|e|e.to_string())?;if jobs.len()<8{jobs.spawn(serve(socket,uid,token.clone(),stop_rx.clone()));}}
            }
        }
        drop(listener);
        let _ = std::fs::remove_file(path);
        while jobs.join_next().await.is_some() {}
        lifetime.abort();
        Ok(())
    }
    /// Unprivileged proxy preserves ordinary Child stdin/stdout/stderr semantics.
    pub async fn client(path: &str, token: &str, args: Vec<String>) -> Result<i32, String> {
        if !allowed(&args) {
            return Err("Unsupported broker operation".into());
        }
        let socket = UnixStream::connect(path).await.map_err(|e| e.to_string())?;
        if socket.peer_cred().map_err(|e| e.to_string())?.uid() != 0 {
            return Err("Broker is not root-owned".into());
        }
        let (read, mut write) = socket.into_split();
        let mut reader = BufReader::new(read);
        let mut header = serde_json::to_vec(&Request {
            token: token.into(),
            args,
        })
        .map_err(|e| e.to_string())?;
        header.push(b'\n');
        write.write_all(&header).await.map_err(|e| e.to_string())?;
        let input = tokio::spawn(async move {
            let mut stdin = tokio::io::stdin();
            let mut data = [0; 4096];
            loop {
                let size = stdin.read(&mut data).await.unwrap_or(0);
                let frame = Frame {
                    stream: if size == 0 { "eof" } else { "stdin" }.into(),
                    data: data[..size].to_vec(),
                    code: None,
                };
                if send(&mut write, &frame).await.is_err() || size == 0 {
                    break;
                }
            }
        });
        let mut stdout = tokio::io::stdout();
        let mut stderr = tokio::io::stderr();
        loop {
            let bytes = line(&mut reader, 32768).await?;
            let frame: Frame = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            match frame.stream.as_str() {
                "stdout" => {
                    stdout
                        .write_all(&frame.data)
                        .await
                        .map_err(|e| e.to_string())?;
                    stdout.flush().await.map_err(|e| e.to_string())?
                }
                "stderr" => {
                    stderr
                        .write_all(&frame.data)
                        .await
                        .map_err(|e| e.to_string())?;
                    stderr.flush().await.map_err(|e| e.to_string())?
                }
                "exit" => {
                    input.abort();
                    return Ok(frame.code.unwrap_or(1));
                }
                _ => return Err("Invalid broker frame".into()),
            }
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn mac_connection_consent_is_a_fixed_optional_flag() {
            let mut args = vec![
                "mac-profile".to_string(),
                "wlan0".into(),
                "private".into(),
                uuid::Uuid::new_v4().to_string(),
            ];
            assert!(allowed(&args));
            args.push("exclusive".into());
            assert!(allowed(&args));
            *args.last_mut().unwrap() = "anything".into();
            assert!(!allowed(&args));
            *args.last_mut().unwrap() = "dedicated".into();
            assert!(allowed(&args));
            args.push("extra".into());
            assert!(!allowed(&args));
        }
        #[test]
        fn broker_denies_arbitrary_commands_and_paths() {
            for args in [
                vec!["bash", "-c", "id"],
                vec!["broker", "token"],
                vec!["install-dependencies", "curl"],
                vec!["mac-profile", "../eth0", "private", "token"],
            ] {
                assert!(!allowed(
                    &args.into_iter().map(String::from).collect::<Vec<_>>()
                ));
            }
            assert!(allowed(&["adapter-report".into()]));
        }
    }
}
