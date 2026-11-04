//! Process integration with the distribution's airodump-ng executable.
//! Capture files exist only in a verified tmpfs directory, never at a GUI path.
#[cfg(target_os = "linux")]
pub mod linux {
    use crate::packets::{read_pcap_record, RawFrame};
    use std::{
        fs::{self, File, OpenOptions},
        io::{self, Read},
        os::unix::{
            fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
            io::AsRawFd,
            process::CommandExt,
        },
        path::{Path, PathBuf},
        process::{Child, ChildStderr, Command, Stdio},
        time::{Duration, Instant},
    };
    pub struct Airodump {
        child: Child,
        directory: PathBuf,
        file: Option<File>,
        offset: u64,
        stopped: bool,
        stderr: ChildStderr,
        diagnostic: std::collections::VecDeque<u8>,
        records_read: u64,
    }
    impl Airodump {
        pub fn start(
            directory: PathBuf,
            interface: &str,
            bssid: &str,
            frequency: u32,
        ) -> Result<Self, String> {
            let name = std::ffi::CString::new(
                directory
                    .parent()
                    .ok_or("Missing runtime directory")?
                    .to_string_lossy()
                    .as_bytes(),
            )
            .map_err(|_| "Invalid runtime path")?;
            let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
            if unsafe { libc::statfs(name.as_ptr(), &mut stat) } != 0
                || stat.f_type != libc::TMPFS_MAGIC
            {
                return Err(
                    "Aircrack capture requires /run on tmpfs for bounded memory-backed recording"
                        .into(),
                );
            }
            fs::create_dir(&directory).map_err(|e| e.to_string())?;
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
            let mut command = Command::new(crate::process::executable("airodump-ng")?);
            command
                .args([
                    "--bssid",
                    bssid,
                    "--beacons",
                    "-C",
                    &frequency.to_string(),
                    "--output-format",
                    "pcap",
                    "--write-interval",
                    "1",
                    "--background",
                    "1",
                    "--write",
                ])
                .arg(directory.join("capture"))
                .arg(interface)
                .env_clear()
                .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
                .env("LC_ALL", "C")
                .env("TERM", "dumb")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped());
            unsafe {
                command.pre_exec(|| {
                    let parent = libc::getppid();
                    if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                        return Err(io::Error::last_os_error());
                    }
                    if parent == 1 || libc::getppid() != parent {
                        return Err(io::Error::other("Helper exited"));
                    }
                    Ok(())
                });
            }
            let mut child = command.spawn().map_err(|e| e.to_string())?;
            let stderr = child
                .stderr
                .take()
                .ok_or("Missing airodump-ng diagnostics")?;
            let fd = stderr.as_raw_fd();
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
            if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
            {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Could not configure bounded airodump-ng diagnostics".into());
            }
            Ok(Self {
                child,
                directory,
                file: None,
                offset: 0,
                stopped: false,
                stderr,
                diagnostic: std::collections::VecDeque::new(),
                records_read: 0,
            })
        }
        fn drain_diagnostics(&mut self) {
            let mut buffer = [0; 1024];
            // Drain without blocking or allowing an unlimited per-poll read.
            for _ in 0..16 {
                match self.stderr.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        self.diagnostic.extend(&buffer[..n]);
                        while self.diagnostic.len() > 4096 {
                            self.diagnostic.pop_front();
                        }
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
        }
        pub fn next(&mut self) -> Result<Option<RawFrame>, String> {
            self.drain_diagnostics();
            if !self.stopped {
                if let Some(status) = self.child.try_wait().map_err(|e| e.to_string())? {
                    self.stopped = true;
                    self.drain_diagnostics();
                    let bytes: Vec<u8> = self.diagnostic.iter().copied().collect();
                    let detail: String = String::from_utf8_lossy(&bytes)
                        .chars()
                        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
                        .take(1500)
                        .collect();
                    return Err(format!(
                        "airodump-ng exited unexpectedly: {status}. {}",
                        if detail.trim().is_empty() {
                            "No stderr output. Check rfkill, driver and target-frequency support."
                        } else {
                            detail.trim()
                        }
                    ));
                }
            }
            if self.file.is_none() {
                match OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_NOFOLLOW)
                    .open(self.directory.join("capture-01.cap"))
                {
                    Ok(f) => self.file = Some(f),
                    Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
                    Err(e) => return Err(e.to_string()),
                }
            }
            let file = self.file.as_mut().unwrap();
            if file.metadata().map_err(|e| e.to_string())?.len() > 16 * 1024 * 1024 {
                return Err("16 MiB RAM capture limit reached; capture stopped".into());
            }
            let record = read_pcap_record(file, &mut self.offset)?;
            if record.is_some() {
                self.records_read = self.records_read.saturating_add(1);
            }
            Ok(record)
        }
        pub fn report(
            &self,
            actual_frequency: Option<u32>,
        ) -> crate::capture_health::CollectorReport {
            crate::capture_health::CollectorReport {
                records_read: self.records_read,
                pcap_bytes: self
                    .file
                    .as_ref()
                    .and_then(|f| f.metadata().ok())
                    .map(|m| m.len()),
                actual_frequency,
            }
        }
        pub fn stop(&mut self) -> Result<(), String> {
            if self.stopped {
                return Ok(());
            }
            unsafe {
                libc::kill(self.child.id() as i32, libc::SIGINT);
            }
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                self.drain_diagnostics();
                if self.child.try_wait().map_err(|e| e.to_string())?.is_some() {
                    self.stopped = true;
                    return Ok(());
                }
                if Instant::now() > deadline {
                    self.child.kill().map_err(|e| e.to_string())?;
                    self.child.wait().map_err(|e| e.to_string())?;
                    self.stopped = true;
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(20))
            }
        }
    }
    impl Drop for Airodump {
        fn drop(&mut self) {
            let _ = self.stop();
        }
    }
    pub fn remove_runtime(directory: &Path) -> Result<(), String> {
        if !directory.exists() {
            return Ok(());
        }
        let m = fs::symlink_metadata(directory).map_err(|e| e.to_string())?;
        if !m.is_dir() || m.uid() != 0 {
            return Err("Unexpected capture runtime identity".into());
        }
        for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_name() != "capture-01.cap" {
                return Err("Unexpected capture file retained for inspection".into());
            }
            let m = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
            if !m.is_file() || m.nlink() != 1 || m.uid() != 0 {
                return Err("Refusing linked capture file".into());
            }
            fs::remove_file(entry.path()).map_err(|e| e.to_string())?
        }
        fs::remove_dir(directory).map_err(|e| e.to_string())
    }
}
