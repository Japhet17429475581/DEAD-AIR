//! Narrow polkit entry point. Never accepts a shell command or destination path.
#[cfg(target_os = "linux")]
mod linux {
    use dead_air::{
        capture_adapter::{interface_mode, CaptureAdapterMode, MonitorChannel},
        model::{parse_mac, valid_interface},
        packets,
    };
    use fs2::FileExt;
    use serde::{Deserialize, Serialize};
    use std::{
        fs::{self, OpenOptions},
        io::{Read, Write},
        os::unix::fs::{MetadataExt, OpenOptionsExt},
        path::PathBuf,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        time::{Duration, Instant},
    };
    #[derive(Serialize, Deserialize)]
    struct Journal {
        uid: u32,
        pid: u32,
        start: String,
        interface: String,
        ifindex: u32,
        phy: String,
        source: String,
        source_index: u32,
        source_up: bool,
        source_managed: bool,
        #[serde(default)]
        monitor_on_source: bool,
        #[serde(default)]
        borrowed_monitor: bool,
        #[serde(default)]
        source_channel: Option<MonitorChannel>,
        #[serde(default)]
        source_connection: Option<String>,
        #[serde(default = "default_autoconnect")]
        source_autoconnect: bool,
    }
    fn default_autoconnect() -> bool {
        true
    }
    fn snapshot(name: &str) -> Result<dead_air::network::ConnectionSnapshot, String> {
        tokio::runtime::Runtime::new()
            .map_err(|e| e.to_string())?
            .block_on(dead_air::network::connection_snapshot(name))
    }
    fn disconnect(name: &str) -> Result<(), String> {
        tokio::runtime::Runtime::new()
            .map_err(|e| e.to_string())?
            .block_on(dead_air::network::disconnect_for_capture(name))
    }
    fn reconnect(name: &str, autoconnect: bool, uuid: Option<&str>) -> Result<(), String> {
        tokio::runtime::Runtime::new()
            .map_err(|e| e.to_string())?
            .block_on(dead_air::network::restore_connection(
                name,
                autoconnect,
                uuid,
            ))
    }
    #[derive(Serialize, Deserialize)]
    struct MacJournal {
        uid: u32,
        pid: u32,
        start: String,
        interface: String,
        ifindex: u32,
        phy: String,
        original: String,
        assigned: String,
        up: bool,
        managed: bool,
        #[serde(default)]
        connection: Option<String>,
        #[serde(default = "default_autoconnect")]
        autoconnect: bool,
    }
    struct MacGuard {
        path: PathBuf,
        record: MacJournal,
    }
    impl MacGuard {
        fn restore(&self) -> Result<(), String> {
            if !self.path.exists() {
                return Ok(());
            }
            let r = &self.record;
            if index(&r.interface) != Some(r.ifindex) || phy_of(&r.interface)? != r.phy {
                return Err(
                    "MAC restoration: adapter identity changed; inspect recovery record".into(),
                );
            }
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(self.path.parent().unwrap().join(format!("{}.lock", r.phy)))
                .map_err(|e| e.to_string())?;
            let deadline = Instant::now() + Duration::from_secs(20);
            while lock.try_lock_exclusive().is_err() {
                if Instant::now() > deadline {
                    return Err("MAC restoration: radio is still reserved by capture".into());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            idle_radio(&r.phy, &r.interface)?;
            if dead_air::mac::current_mac(&r.interface)? == r.original {
                let state = snapshot(&r.interface)?;
                if state.state == 100 && state.uuid == r.connection {
                    reconnect(&r.interface, r.autoconnect, r.connection.as_deref())?;
                    fs::remove_file(&self.path).map_err(|e| e.to_string())?;
                    return Ok(());
                }
            }
            control(&r.interface, None, true)?;
            if output("iw", &["dev", &r.interface, "link"])?.contains("Connected to") {
                return Err("MAC restoration refused on a connected adapter; disconnect it and retry recovery".into());
            }
            control(&r.interface, Some(false), true)?;
            let result = (|| {
                run("ip", &["link", "set", "dev", &r.interface, "down"])?;
                run(
                    "ip",
                    &["link", "set", "dev", &r.interface, "address", &r.original],
                )?;
                Ok::<(), String>(())
            })();
            let link = run(
                "ip",
                &[
                    "link",
                    "set",
                    "dev",
                    &r.interface,
                    if r.up { "up" } else { "down" },
                ],
            );
            let managed = control(&r.interface, Some(r.managed), false);
            result?;
            link?;
            managed?;
            if dead_air::mac::current_mac(&r.interface)? != r.original {
                return Err("MAC restoration readback did not match original address".into());
            }
            reconnect(&r.interface, r.autoconnect, r.connection.as_deref())?;
            fs::remove_file(&self.path).map_err(|e| e.to_string())
        }
    }
    impl Drop for MacGuard {
        fn drop(&mut self) {
            if let Err(e) = self.restore() {
                eprintln!("MAC restoration required: {e}");
            }
        }
    }
    fn mac_profile(
        root: &std::path::Path,
        uid: u32,
        source: &str,
        profile: &str,
        token: &str,
        allow_disconnect: bool,
    ) -> Result<(), String> {
        if !valid_interface(source) {
            return Err("Invalid interface".into());
        }
        let token = uuid::Uuid::parse_str(token).map_err(|_| "Invalid operation token")?;
        let assigned = dead_air::mac::profile_mac(profile)?;
        let phy = phy_of(source)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(root.join(format!("{phy}.lock")))
            .map_err(|e| e.to_string())?;
        lock.try_lock_exclusive()
            .map_err(|_| "Stop capture before changing MAC")?;
        idle_radio(&phy, source)?;
        if !output("iw", &["dev", source, "info"])?
            .lines()
            .any(|l| l.trim() == "type managed")
        {
            return Err("MAC profiles require an idle managed Wi-Fi adapter".into());
        }
        let before = snapshot(source)?;
        dead_air::mac::validate_connection_state(before.state, allow_disconnect)?;
        if output("iw", &["dev", source, "link"])?.contains("Connected to") && before.state != 100 {
            return Err("The connected card is not controlled by a restorable NetworkManager profile; disconnect it explicitly first".into());
        }
        if before.state == 100 && before.uuid.is_none() {
            return Err("Cannot save the active Wi-Fi profile for MAC restoration".into());
        }
        // An unresolved earlier operation must be recovered, not overwritten.
        for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("mac") {
                continue;
            }
            let record: MacJournal = serde_json::from_reader(
                OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_NOFOLLOW)
                    .open(path)
                    .map_err(|e| e.to_string())?
                    .take(8192),
            )
            .map_err(|e| e.to_string())?;
            if record.phy == phy {
                return Err("An existing MAC lease or recovery record owns this radio".into());
            }
        }
        let original = dead_air::mac::current_mac(source)?;
        let flags = fs::read_to_string(format!("/sys/class/net/{source}/flags"))
            .map_err(|e| e.to_string())?;
        let up = u32::from_str_radix(flags.trim().trim_start_matches("0x"), 16)
            .map_err(|e| e.to_string())?
            & 1
            != 0;
        let managed = before.managed;
        let path = root.join(format!("{token}.mac"));
        let record = MacJournal {
            uid,
            pid: std::process::id(),
            start: proc_start(std::process::id()),
            interface: source.into(),
            ifindex: index(source).ok_or("Adapter disappeared")?,
            phy,
            original: original.clone(),
            assigned: assigned.clone(),
            up,
            managed,
            connection: before.uuid,
            autoconnect: before.autoconnect,
        };
        let mut journal = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)
            .map_err(|e| e.to_string())?;
        journal
            .write_all(&serde_json::to_vec(&record).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        journal.sync_all().map_err(|e| e.to_string())?;
        let guard = MacGuard { path, record };
        let changed = (|| {
            if before.state == 100 {
                disconnect(source)?;
            }
            control(source, Some(false), true)?;
            run("ip", &["link", "set", "dev", source, "down"])?;
            run("ip", &["link", "set", "dev", source, "address", &assigned])?;
            run(
                "ip",
                &["link", "set", "dev", source, if up { "up" } else { "down" }],
            )?;
            // Keep NetworkManager released until the lease is restored.
            // Re-enabling it here can immediately replace the assigned MAC.
            if dead_air::mac::current_mac(source)? != assigned {
                return Err(
                    "MAC assignment readback mismatch; NetworkManager or driver replaced it".into(),
                );
            }
            Ok::<(), String>(())
        })();
        drop(lock);
        changed?;
        let stopped = Arc::new(AtomicBool::new(false));
        for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
            signal_hook::flag::register(signal, stopped.clone()).map_err(|e| e.to_string())?;
        }
        send(serde_json::json!({"kind":"mac-ready","original":original,"assigned":assigned}))?;
        while !stopped.load(Ordering::Relaxed) {
            let mut poll = libc::pollfd {
                fd: 0,
                events: libc::POLLIN | libc::POLLHUP,
                revents: 0,
            };
            unsafe {
                libc::poll(&mut poll, 1, 200);
            }
            if poll.revents & (libc::POLLHUP | libc::POLLERR) != 0 {
                break;
            }
            if poll.revents & libc::POLLIN != 0 {
                let mut b = [0u8; 1];
                if unsafe { libc::read(0, b.as_mut_ptr().cast(), 1) } <= 0 || b[0] == b'Q' {
                    break;
                }
            }
        }
        guard.restore()?;
        send(serde_json::json!({"kind":"mac-restored"}))
    }
    fn mac_recover(root: &std::path::Path, uid: u32) -> Result<(), String> {
        let mut count = 0;
        for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("mac") {
                continue;
            }
            let record: MacJournal = serde_json::from_reader(
                OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_NOFOLLOW)
                    .open(&path)
                    .map_err(|e| e.to_string())?
                    .take(8192),
            )
            .map_err(|e| e.to_string())?;
            if record.uid != uid {
                continue;
            }
            if proc_start(record.pid) == record.start && !record.start.is_empty() {
                return Err("MAC helper still active; stop its lease first".into());
            }
            if !valid_interface(&record.interface)
                || !record.phy.starts_with("phy")
                || !record.phy[3..].bytes().all(|b| b.is_ascii_digit())
                || parse_mac(&record.original).is_err()
            {
                return Err("Invalid MAC recovery record".into());
            }
            MacGuard { path, record }.restore()?;
            count += 1;
        }
        println!("Recovered {count} MAC operation(s).");
        Ok(())
    }
    fn control(name: &str, managed: Option<bool>, idle: bool) -> Result<bool, String> {
        tokio::runtime::Runtime::new()
            .map_err(|e| e.to_string())?
            .block_on(dead_air::network::adapter_control(name, managed, idle))
    }
    fn output(tool: &str, args: &[&str]) -> Result<String, String> {
        tokio::runtime::Runtime::new()
            .map_err(|e| e.to_string())?
            .block_on(dead_air::process::output(tool, args, 6))
    }
    fn run(tool: &str, args: &[&str]) -> Result<(), String> {
        let mut command = dead_air::process::command(tool)?;
        command
            .args(args)
            .env_clear()
            .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
            .env("LC_ALL", "C");
        let operation = format!("{tool} {}", args.join(" "));
        tokio::runtime::Runtime::new()
            .map_err(|e| e.to_string())?
            .block_on(dead_air::process::output_command(command, &operation, 6))
            .map(|_| ())
    }
    fn proc_start(pid: u32) -> String {
        fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .and_then(|s| {
                s.rsplit_once(") ")
                    .map(|(_, tail)| tail.split_whitespace().nth(19).unwrap_or("").to_string())
            })
            .unwrap_or_default()
    }
    fn phy_of(name: &str) -> Result<String, String> {
        let p = fs::canonicalize(format!("/sys/class/net/{name}/phy80211"))
            .map_err(|e| format!("Radio {name}: {e}"))?;
        p.file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .ok_or("No physical radio".into())
    }
    fn idle_radio(phy: &str, except: &str) -> Result<(), String> {
        for entry in fs::read_dir("/sys/class/net").map_err(|e| e.to_string())? {
            let name = entry
                .map_err(|e| e.to_string())?
                .file_name()
                .to_string_lossy()
                .into_owned();
            if name == except {
                continue;
            }
            if phy_of(&name).ok().as_deref() == Some(phy) {
                let flags = fs::read_to_string(format!("/sys/class/net/{name}/flags"))
                    .map_err(|e| e.to_string())?;
                let flags = u32::from_str_radix(flags.trim().trim_start_matches("0x"), 16)
                    .map_err(|e| e.to_string())?;
                if flags & 1 != 0 {
                    return Err(format!("Radio {phy} is in use by {name}. Other active interfaces on this radio are not modified; stop them explicitly or select another radio."));
                }
            }
        }
        Ok(())
    }
    fn index(name: &str) -> Option<u32> {
        fs::read_to_string(format!("/sys/class/net/{name}/ifindex"))
            .ok()?
            .trim()
            .parse()
            .ok()
    }
    struct Guard {
        journal: PathBuf,
        record: Journal,
        owned: std::cell::Cell<bool>,
    }
    fn recovery_lock(root: &std::path::Path, record: &Journal) -> Result<std::fs::File, String> {
        if !record.phy.starts_with("phy")
            || !record.phy[3..].bytes().all(|b| b.is_ascii_digit())
            || !valid_interface(&record.source)
        {
            return Err("Invalid recovery radio identity".into());
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(root.join(format!("{}.lock", record.phy)))
            .map_err(|e| e.to_string())?;
        file.try_lock_exclusive()
            .map_err(|_| "Radio is reserved by an active capture")?;
        Ok(file)
    }
    impl Guard {
        fn cleanup(&self) -> Result<(), String> {
            if !self.owned.get() || !self.journal.exists() {
                return Ok(());
            }
            if self.record.borrowed_monitor {
                return self.restore_borrowed_monitor();
            }
            match index(&self.record.interface) {
                None => (),
                Some(i) => {
                    if self.record.ifindex == 0
                        || i != self.record.ifindex
                        || phy_of(&self.record.interface)? != self.record.phy
                    {
                        return Err("Interface identity changed; refusing automatic deletion. Inspect recovery journal.".into());
                    }
                    if self.record.monitor_on_source {
                        let mode = output("iw", &["dev", &self.record.source, "info"])?;
                        // NetworkManager may not expose an interface of type monitor.
                        // Restore its kernel type before asking D-Bus to reactivate it.
                        if mode.lines().any(|line| line.trim() == "type monitor") {
                            run("ip", &["link", "set", "dev", &self.record.source, "down"])?;
                            run(
                                "iw",
                                &["dev", &self.record.source, "set", "type", "managed"],
                            )?;
                        } else if !mode.lines().any(|line| line.trim() == "type managed") {
                            return Err(
                                "Capture interface mode changed externally; restoration refused"
                                    .into(),
                            );
                        }
                    } else {
                        run("iw", &["dev", &self.record.interface, "del"])?
                    }
                }
            }
            dead_air::aircrack::linux::remove_runtime(&self.journal.with_extension("capture"))?;
            if index(&self.record.source) != Some(self.record.source_index)
                || phy_of(&self.record.source)? != self.record.phy
            {
                return Err(
                    "Original adapter identity changed; restoration requires inspection".into(),
                );
            }
            let retry = Instant::now();
            let state = loop {
                match snapshot(&self.record.source) {
                    Ok(state) => break state,
                    Err(error) => {
                        if retry.elapsed() > Duration::from_secs(3) {
                            return Err(error);
                        }
                        std::thread::sleep(Duration::from_millis(150));
                    }
                }
            };
            if state.state == 100 {
                if state.uuid != self.record.source_connection {
                    return Err(
                        "Another connection owns the restored interface; inspect recovery journal"
                            .into(),
                    );
                }
                reconnect(
                    &self.record.source,
                    self.record.source_autoconnect,
                    self.record.source_connection.as_deref(),
                )?;
                fs::remove_file(&self.journal).map_err(|e| e.to_string())?;
                return Ok(());
            }
            control(&self.record.source, None, true)?;
            run(
                "ip",
                &[
                    "link",
                    "set",
                    "dev",
                    &self.record.source,
                    if self.record.source_up { "up" } else { "down" },
                ],
            )?;
            control(&self.record.source, Some(self.record.source_managed), false)?;
            reconnect(
                &self.record.source,
                self.record.source_autoconnect,
                self.record.source_connection.as_deref(),
            )?;
            if self.journal.exists() {
                fs::remove_file(&self.journal).map_err(|e| e.to_string())?
            }
            Ok(())
        }
        fn restore_borrowed_monitor(&self) -> Result<(), String> {
            let record = &self.record;
            if record.interface != record.source
                || record.ifindex != record.source_index
                || index(&record.source) != Some(record.source_index)
                || phy_of(&record.source)? != record.phy
            {
                return Err(
                    "Borrowed monitor interface identity changed; inspect recovery journal".into(),
                );
            }
            let info = output("iw", &["dev", &record.source, "info"])?;
            if interface_mode(&info)? != CaptureAdapterMode::ExistingMonitor {
                return Err(
                    "Borrowed interface mode changed externally; automatic restoration refused"
                        .into(),
                );
            }
            idle_radio(&record.phy, &record.source)?;
            if let Some(channel) = &record.source_channel {
                let arguments = channel.arguments()?;
                run("ip", &["link", "set", "dev", &record.source, "up"])?;
                let mut args = vec!["dev", record.source.as_str(), "set", "freq"];
                args.extend(arguments.iter().map(String::as_str));
                run("iw", &args)?;
                let restored = output("iw", &["dev", &record.source, "info"])?;
                if MonitorChannel::from_info(&restored)?.as_ref() != Some(channel) {
                    return Err("The driver did not restore the original monitor channel; inspect recovery journal".into());
                }
            }
            run(
                "ip",
                &[
                    "link",
                    "set",
                    "dev",
                    &record.source,
                    if record.source_up { "up" } else { "down" },
                ],
            )?;
            dead_air::aircrack::linux::remove_runtime(&self.journal.with_extension("capture"))?;
            fs::remove_file(&self.journal).map_err(|e| e.to_string())
        }
        fn save(&self) -> Result<(), String> {
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(!self.owned.get())
                .truncate(self.owned.get())
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&self.journal)
                .map_err(|e| e.to_string())?;
            self.owned.set(true);
            f.write_all(&serde_json::to_vec(&self.record).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            f.sync_all().map_err(|e| e.to_string())
        }
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            if let Err(e) = self.cleanup() {
                eprintln!("Cleanup required: {e}")
            }
        }
    }
    fn send(value: serde_json::Value) -> Result<(), String> {
        let mut bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        unsafe {
            let flags = libc::fcntl(1, libc::F_GETFL);
            libc::fcntl(1, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut offset = 0;
        while offset < bytes.len() {
            let n =
                unsafe { libc::write(1, bytes[offset..].as_ptr().cast(), bytes.len() - offset) };
            if n > 0 {
                offset += n as usize;
                continue;
            }
            let e = std::io::Error::last_os_error();
            if e.kind() != std::io::ErrorKind::WouldBlock {
                return Err(format!("Capture receiver closed: {e}"));
            }
            if Instant::now() > deadline {
                return Err("Capture receiver stalled; stopping and restoring interface".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn old_mac_journals_do_not_invent_a_saved_connection() {
            let record:MacJournal=serde_json::from_value(serde_json::json!({"uid":1000,"pid":1,"start":"fixture","interface":"wlan0","ifindex":2,"phy":"phy0","original":"02:00:00:00:00:01","assigned":"02:00:00:00:00:02","up":true,"managed":true})).unwrap();
            assert!(record.connection.is_none());
            assert!(record.autoconnect);
        }
        #[test]
        fn legacy_journal_keeps_original_recovery_semantics() {
            let record: Journal = serde_json::from_value(serde_json::json!({
                "uid":1000,"pid":1,"start":"test","interface":"da123","ifindex":3,
                "phy":"phy0","source":"wlan0","source_index":2,"source_up":true,"source_managed":true
            })).unwrap();
            assert!(!record.borrowed_monitor);
            assert!(!record.monitor_on_source);
            assert!(record.source_channel.is_none());
        }
        #[test]
        fn command_failure_preserves_operation_stderr_and_exit_status() {
            let error = run(
                "sh",
                &["-c", "printf 'fixture driver busy (-16)' >&2; exit 7"],
            )
            .unwrap_err();
            assert!(error.contains("fixture driver busy (-16)"), "{error}");
            assert!(error.contains("exit status: 7"), "{error}");
        }
        #[test]
        fn capture_token_collision_does_not_overwrite_or_remove_existing_journal() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("existing.json");
            fs::write(&path, b"keep existing record").unwrap();
            let guard = Guard {
                journal: path.clone(),
                owned: std::cell::Cell::new(false),
                record: Journal {
                    uid: 0,
                    pid: 0,
                    start: String::new(),
                    interface: "da-test".into(),
                    ifindex: 0,
                    phy: "phy0".into(),
                    source: "none0".into(),
                    source_index: 0,
                    source_up: false,
                    source_managed: false,
                    monitor_on_source: false,
                    borrowed_monitor: false,
                    source_channel: None,
                    source_connection: None,
                    source_autoconnect: true,
                },
            };
            assert!(guard.save().is_err());
            drop(guard);
            assert_eq!(fs::read(path).unwrap(), b"keep existing record");
        }
    }
    pub fn main() -> Result<(), String> {
        if unsafe { libc::geteuid() } != 0 {
            return Err("This helper must be authorized by polkit".into());
        }
        let uid: u32 = std::env::var("PKEXEC_UID")
            .map_err(|_| "Invoke the installed helper through pkexec")?
            .parse()
            .map_err(|_| "Invalid polkit user")?;
        let args: Vec<String> = std::env::args().collect();
        if args.len() == 3 && args[1] == "broker" {
            return tokio::runtime::Runtime::new()
                .map_err(|e| e.to_string())?
                .block_on(dead_air::broker::linux::daemon(uid, args[2].clone()));
        }
        if args.len() == 3 && args[1] == "install-dependencies" {
            dead_air::setup::install(&args[2])?;
            println!("Dependencies installed.");
            return Ok(());
        }
        if args.len() == 2 && args[1] == "adapter-report" {
            print!("{}", output("airmon-ng", &[])?);
            return Ok(());
        }
        let root = PathBuf::from("/run/deadair");
        if !root.exists() {
            fs::create_dir(&root).map_err(|e| e.to_string())?;
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?
        }
        let meta = fs::symlink_metadata(&root).map_err(|e| e.to_string())?;
        if !meta.is_dir() || meta.uid() != 0 || meta.mode() & 0o077 != 0 {
            return Err("Unsafe helper runtime directory".into());
        }
        if (args.len() == 5 || args.len() == 6) && args[1] == "mac-profile" {
            let consent = match args.get(5).map(String::as_str) {
                None | Some("dedicated") => false,
                Some("exclusive") => true,
                _ => return Err("Invalid MAC connection mode".into()),
            };
            return mac_profile(&root, uid, &args[2], &args[3], &args[4], consent);
        }
        if args.len() == 2 && args[1] == "mac-recover" {
            return mac_recover(&root, uid);
        }
        if args.len() == 2 && args[1] == "recover-owned" {
            let mut count = 0;
            for entry in fs::read_dir(&root).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                if entry.path().extension().and_then(|x| x.to_str()) != Some("json") {
                    continue;
                }
                let path = entry.path();
                let f = OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_NOFOLLOW)
                    .open(&path)
                    .map_err(|e| e.to_string())?;
                let mut bytes = Vec::new();
                f.take(8192)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                let record: Journal = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                if record.uid != uid {
                    continue;
                }
                if proc_start(record.pid) == record.start && !record.start.is_empty() {
                    return Err(
                        "An owned capture helper is still active; stop it before recovery".into(),
                    );
                }
                if !valid_interface(&record.interface)
                    || !(record.interface.starts_with("da")
                        || (record.monitor_on_source || record.borrowed_monitor)
                            && record.interface == record.source)
                    || !valid_interface(&record.source)
                {
                    return Err("Invalid recovery record".into());
                }
                let _lock = recovery_lock(&root, &record)?;
                let guard = Guard {
                    journal: path,
                    record,
                    owned: std::cell::Cell::new(true),
                };
                guard.cleanup()?;
                count += 1;
            }
            println!("Recovered {count} app-owned capture operation(s).");
            return Ok(());
        }
        if args.len() == 3 && args[1] == "recover" {
            let token = uuid::Uuid::parse_str(&args[2]).map_err(|_| "Invalid recovery token")?;
            let path = root.join(format!("{token}.json"));
            let mut f = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&path)
                .map_err(|e| e.to_string())?;
            let mut bytes = Vec::new();
            Read::by_ref(&mut f)
                .take(8192)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            let record: Journal = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if record.uid != uid
                || !(record.interface.starts_with("da")
                    || (record.monitor_on_source || record.borrowed_monitor)
                        && record.interface == record.source)
                || !valid_interface(&record.interface)
                || proc_start(record.pid) == record.start && !record.start.is_empty()
            {
                return Err("Journal belongs to another user or an active helper".into());
            }
            let _lock = recovery_lock(&root, &record)?;
            let guard = Guard {
                journal: path,
                record,
                owned: std::cell::Cell::new(true),
            };
            guard.cleanup()?;
            return Ok(());
        }
        if !(args.len() == 6 || args.len() == 7) || args[1] != "capture" {
            return Err(
                "Usage: helper capture INTERFACE BSSID FREQUENCY TOKEN | recover TOKEN".into(),
            );
        }
        let source = &args[2];
        let exclusive = match args.get(6).map(String::as_str) {
            Some("exclusive") => true,
            None | Some("dedicated") => false,
            _ => return Err("Invalid capture mode".into()),
        };
        if !valid_interface(source) {
            return Err("Invalid interface".into());
        }
        let bssid = parse_mac(&args[3])?;
        let frequency: u32 = args[4].parse().map_err(|_| "Invalid frequency")?;
        if !((2412..=2484).contains(&frequency)
            || (5000..=5895).contains(&frequency)
            || (5955..=7115).contains(&frequency))
        {
            return Err("Unsupported radio frequency".into());
        }
        let token = uuid::Uuid::parse_str(&args[5]).map_err(|_| "Invalid operation token")?;
        let phy = phy_of(source)?;
        let phy_lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(root.join(format!("{phy}.lock")))
            .map_err(|e| e.to_string())?;
        phy_lock
            .try_lock_exclusive()
            .map_err(|_| "This radio is already reserved by DEAD//AIR")?;
        // Do not borrow an interface whose original managed state is still in
        // an earlier app journal. Recovery must complete before another lease.
        for entry in fs::read_dir(&root).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let record: Journal = serde_json::from_reader(
                OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_NOFOLLOW)
                    .open(path)
                    .map_err(|e| e.to_string())?
                    .take(8192),
            )
            .map_err(|e| format!("Inspect invalid capture recovery journal: {e}"))?;
            if record.phy == phy {
                return Err("This radio has an unfinished DEAD//AIR capture. Use ADAPTERS > RESTORE STALE CAPTURE INTERFACES before starting again.".into());
            }
        }
        idle_radio(&phy, source)?;
        let info = output("iw", &["dev", source, "info"])?;
        let borrowed_monitor = interface_mode(&info)? == CaptureAdapterMode::ExistingMonitor;
        if !borrowed_monitor
            && !exclusive
            && output("iw", &["dev", source, "link"])?.contains("Connected to")
        {
            return Err("Selected adapter has an active connection".into());
        }
        // Existing monitor interfaces need not be known to NetworkManager.
        // Do not conceal a managed-card D-Bus failure with this branch.
        let before = if borrowed_monitor {
            None
        } else {
            Some(snapshot(source)?)
        };
        if before
            .as_ref()
            .is_some_and(|state| state.state >= 40 && state.state <= 110 && state.state != 100)
        {
            return Err("Wi-Fi is changing state; wait before capture".into());
        }
        if before.as_ref().is_some_and(|state| state.state == 100) && !exclusive {
            return Err("Explicit temporary-disconnection consent is required".into());
        }
        let source_managed = before.as_ref().is_some_and(|state| state.managed);
        let source_flags = fs::read_to_string(format!("/sys/class/net/{source}/flags"))
            .map_err(|e| e.to_string())?;
        let source_up = u32::from_str_radix(source_flags.trim().trim_start_matches("0x"), 16)
            .map_err(|e| e.to_string())?
            & 1
            != 0;
        let source_channel = if borrowed_monitor {
            MonitorChannel::from_info(&info)?
        } else {
            None
        };
        // Use the selected card itself, including idle cards. Creating a
        // second VIF can fail even when the driver supports monitor mode.
        let interface = source.to_string();
        let stopped = Arc::new(AtomicBool::new(false));
        for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
            signal_hook::flag::register(signal, stopped.clone()).map_err(|e| e.to_string())?;
        }
        let mut guard = Guard {
            journal: root.join(format!("{token}.json")),
            owned: std::cell::Cell::new(false),
            record: Journal {
                uid,
                pid: std::process::id(),
                start: proc_start(std::process::id()),
                interface: interface.clone(),
                ifindex: index(source).ok_or("Source disappeared")?,
                phy: phy.clone(),
                source: source.into(),
                source_index: index(source).ok_or("Source disappeared")?,
                source_up,
                source_managed,
                monitor_on_source: !borrowed_monitor,
                borrowed_monitor,
                source_channel,
                source_connection: before.as_ref().and_then(|state| state.uuid.clone()),
                source_autoconnect: before.as_ref().is_some_and(|state| state.autoconnect),
            },
        };
        guard.save()?;
        if !borrowed_monitor {
            if exclusive {
                disconnect(source)?;
            }
            control(source, Some(false), true)?;
            run("ip", &["link", "set", "dev", source, "down"])?;
            run("iw", &["dev", source, "set", "type", "monitor"])?;
        }
        guard.record.ifindex =
            index(&interface).ok_or("Monitor interface missing after creation")?;
        guard.save()?;
        run("ip", &["link", "set", "dev", &interface, "up"])?;
        run(
            "iw",
            &["dev", &interface, "set", "freq", &frequency.to_string()],
        )?;
        let mut cap = dead_air::aircrack::linux::Airodump::start(
            guard.journal.with_extension("capture"),
            &interface,
            &args[3],
            frequency,
        )?;
        send(
            serde_json::json!({"kind":"ready","interface":interface,"token":token.to_string(),"version":env!("CARGO_PKG_VERSION")}),
        )?;
        let deadline =
            Instant::now() + Duration::from_secs(dead_air::capture_health::CAPTURE_LIMIT_SECONDS);
        let mut check = Instant::now();
        let mut report_at = Instant::now();
        let mut frequency_check_at = Instant::now() - Duration::from_secs(2);
        let mut actual_frequency = None;
        let mut count = 0;
        while !stopped.load(Ordering::Relaxed) && Instant::now() < deadline && count < 20000 {
            let mut poll = libc::pollfd {
                fd: 0,
                events: libc::POLLIN | libc::POLLHUP,
                revents: 0,
            };
            unsafe {
                libc::poll(&mut poll, 1, 0);
            }
            if poll.revents & (libc::POLLHUP | libc::POLLERR) != 0 {
                break;
            }
            if poll.revents & libc::POLLIN != 0 {
                let mut b = [0u8; 1];
                let n = unsafe { libc::read(0, b.as_mut_ptr().cast(), 1) };
                if n <= 0 || b[0] == b'Q' {
                    break;
                }
            }
            if check.elapsed() > Duration::from_millis(500) {
                idle_radio(&phy, &interface)?;
                check = Instant::now()
            }
            match cap.next()? {
                Some(packet) => {
                    if packets::parse(&packet.data, packet.time, bssid).is_some() {
                        send(
                            serde_json::json!({"kind":"packet","time":packet.time,"data":packet.data}),
                        )?;
                        count += 1
                    }
                }
                None => std::thread::sleep(Duration::from_millis(20)),
            }
            if frequency_check_at.elapsed() >= Duration::from_secs(2) {
                let info = output("iw", &["dev", &interface, "info"])?;
                if interface_mode(&info)? != CaptureAdapterMode::ExistingMonitor {
                    return Err(
                        "The capture interface left monitor mode; observation stopped".into(),
                    );
                }
                actual_frequency =
                    MonitorChannel::from_info(&info)?.map(|channel| channel.frequency);
                if actual_frequency.is_some_and(|actual| actual != frequency) {
                    return Err(format!("Capture frequency changed: requested {frequency} MHz, actual {} MHz. Observation stopped; another process or the driver may be changing the channel.", actual_frequency.unwrap()));
                }
                frequency_check_at = Instant::now();
            }
            if report_at.elapsed() >= Duration::from_secs(1) {
                send(serde_json::json!({"kind":"progress","report":cap.report(actual_frequency)}))?;
                report_at = Instant::now();
            }
        }
        cap.stop()?;
        while count < 20000 {
            match cap.next()? {
                Some(packet) => {
                    if packets::parse(&packet.data, packet.time, bssid).is_some() {
                        send(
                            serde_json::json!({"kind":"packet","time":packet.time,"data":packet.data}),
                        )?;
                        count += 1
                    }
                }
                None => break,
            }
        }
        send(serde_json::json!({"kind":"progress","report":cap.report(actual_frequency)}))?;
        drop(cap);
        send(serde_json::json!({"kind":"restoring"}))?;
        guard.cleanup()?;
        send(
            serde_json::json!({"kind":"done","reason":if count>=20000{"frame limit"}else if Instant::now()>=deadline{"five-minute limit"}else{"stopped"}}),
        )?;
        Ok(())
    }
}
fn main() {
    #[cfg(target_os = "linux")]
    {
        let args: Vec<String> = std::env::args().collect();
        if args.get(1).is_some_and(|s| s == "broker-client") {
            if args.len() < 5 {
                eprintln!("Invalid broker client arguments");
                std::process::exit(1);
            }
            let runtime = tokio::runtime::Runtime::new().expect("Runtime initialization");
            let result = runtime.block_on(dead_air::broker::linux::client(
                &args[2],
                &args[3],
                args[4..].to_vec(),
            ));
            // Avoid waiting for a blocked stdin reader after the remote job exits.
            match result {
                Ok(code) => std::process::exit(code),
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            }
        }
    }
    #[cfg(target_os = "linux")]
    if let Err(error) = linux::main() {
        println!("{}", serde_json::json!({"kind":"error","message":error}));
        std::process::exit(1)
    }
    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("Linux only");
        std::process::exit(1)
    }
}
