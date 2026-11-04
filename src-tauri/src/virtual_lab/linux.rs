//! Root worker for one built-in topology. Namespace FDs, children and netlink
//! sockets are owned by this worker; physical interface names are never inputs.
use super::{
    netlink::GenericNetlink, VirtualStatus, LAB_ID, LAB_PASSPHRASE, LAB_SSID, LIMIT_SECONDS,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
            net::UnixDatagram,
            process::CommandExt,
        },
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

const AP: &str = "daap0";
const STA: &str = "dasta0";
const MON: &str = "damon0";
const ROOT: &str = "/run/deadair/virtual";
const AP_ADDRESS: &str = "10.88.0.1";

fn io_error(context: &str) -> String {
    format!("{context}: {}", std::io::Error::last_os_error())
}
fn enter_namespace(fd: i32) -> Result<(), String> {
    if unsafe { libc::setns(fd, libc::CLONE_NEWNET) } != 0 {
        Err(io_error("Enter owned lab namespace"))
    } else {
        Ok(())
    }
}
fn proc_start(pid: u32) -> String {
    fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|s| {
            s.rsplit_once(") ").and_then(|(_, tail)| {
                if tail.split_whitespace().next() == Some("Z") {
                    None
                } else {
                    Some(tail.split_whitespace().nth(19).unwrap_or("").to_owned())
                }
            })
        })
        .unwrap_or_default()
}
fn send(value: serde_json::Value) -> Result<(), String> {
    let mut bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    if bytes.len() > 24575 {
        return Err("Virtual lab output exceeded bounds".into());
    }
    let mut offset = 0;
    let deadline = Instant::now() + Duration::from_secs(3);
    while offset < bytes.len() {
        let mut p = libc::pollfd {
            fd: 1,
            events: libc::POLLOUT,
            revents: 0,
        };
        unsafe {
            libc::poll(&mut p, 1, 100);
        }
        if p.revents & libc::POLLOUT != 0 {
            let n =
                unsafe { libc::write(1, bytes[offset..].as_ptr().cast(), bytes.len() - offset) };
            if n <= 0 {
                return Err(io_error("Virtual lab receiver closed"));
            }
            offset += n as usize;
        }
        if Instant::now() > deadline {
            return Err("Virtual lab receiver stalled".into());
        }
    }
    Ok(())
}
fn attach_namespace(command: &mut Command, namespace: i32) {
    unsafe {
        command.pre_exec(move || {
            let parent = libc::getppid();
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0
                || libc::setns(namespace, libc::CLONE_NEWNET) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            if parent == 1 || parent != libc::getppid() {
                return Err(std::io::Error::other("Lab owner exited"));
            }
            Ok(())
        });
    }
}
fn output(namespace: i32, tool: &str, args: &[&str], seconds: u64) -> Result<String, String> {
    let mut command = crate::process::command(tool)?;
    command
        .args(args)
        .env_clear()
        .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
        .env("LC_ALL", "C");
    unsafe {
        command.pre_exec(move || {
            if libc::setns(namespace, libc::CLONE_NEWNET) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?
        .block_on(crate::process::output_command(
            command,
            &format!("{tool} {}", args.join(" ")),
            seconds,
        ))
}
fn run(namespace: i32, tool: &str, args: &[&str]) -> Result<(), String> {
    output(namespace, tool, args, 8).map(|_| ())
}
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())
}

#[derive(Serialize, Deserialize)]
struct ProcessRecord {
    pid: u32,
    start: String,
}
#[derive(Serialize, Deserialize)]
struct Journal {
    uid: u32,
    pid: u32,
    start: String,
    token: String,
    children: Vec<ProcessRecord>,
}
impl Journal {
    fn validate(&self) -> Result<(), String> {
        let valid = |pid: u32, start: &str| {
            pid > 1
                && pid <= i32::MAX as u32
                && !start.is_empty()
                && start.bytes().all(|b| b.is_ascii_digit())
        };
        if !valid(self.pid, &self.start)
            || self.children.len() > 3
            || self.children.iter().any(|p| !valid(p.pid, &p.start))
        {
            return Err("Invalid owned process identity in lab journal".into());
        }
        Ok(())
    }
}
struct Resources {
    directory: PathBuf,
    journal: Journal,
    children: Vec<Child>,
    collector: Option<crate::aircrack::linux::Airodump>,
    radio_socket: Option<GenericNetlink>,
    owner: Option<File>,
    station: Option<File>,
    radio_names: Vec<String>,
    cleaned: bool,
}
impl Resources {
    fn save(&self) -> Result<(), String> {
        let path = self.directory.join("manifest.json");
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(|e| e.to_string())?;
        file.write_all(&serde_json::to_vec(&self.journal).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())
    }
    fn track(&mut self, pid: u32) -> Result<(), String> {
        self.journal.children.push(ProcessRecord {
            pid,
            start: proc_start(pid),
        });
        self.save()
    }
    fn spawn(
        &mut self,
        namespace: i32,
        tool: &str,
        args: &[&str],
        log: &str,
    ) -> Result<(), String> {
        let log = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(self.directory.join(log))
            .map_err(|e| e.to_string())?;
        let mut command = Command::new(crate::process::executable(tool)?);
        command
            .args(args)
            .env_clear()
            .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
            .stderr(Stdio::from(log));
        attach_namespace(&mut command, namespace);
        let child = command.spawn().map_err(|e| format!("{tool}: {e}"))?;
        let pid = child.id();
        self.children.push(child);
        self.track(pid)
    }
    fn cleanup(&mut self) -> Result<(), String> {
        if self.cleaned {
            return Ok(());
        }
        if let Some(mut collector) = self.collector.take() {
            collector.stop()?;
            drop(collector);
        }
        for child in &mut self.children {
            if child.try_wait().map_err(|e| e.to_string())?.is_none() {
                unsafe {
                    libc::kill(child.id() as i32, libc::SIGTERM);
                }
            }
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        for child in &mut self.children {
            while child.try_wait().map_err(|e| e.to_string())?.is_none()
                && Instant::now() < deadline
            {
                std::thread::sleep(Duration::from_millis(30));
            }
            if child.try_wait().map_err(|e| e.to_string())?.is_none() {
                child.kill().map_err(|e| e.to_string())?;
            }
            child.wait().map_err(|e| e.to_string())?;
        }
        self.children.clear();
        self.radio_socket.take();
        // Socket-close deletion is a kernel guarantee requested when creating
        // each radio. Verify disappearance before declaring cleanup complete.
        for namespace in [&self.owner, &self.station].into_iter().flatten() {
            let deadline = Instant::now() + Duration::from_secs(4);
            loop {
                let radios = output(namespace.as_raw_fd(), "iw", &["phy"], 5)?;
                if !self
                    .radio_names
                    .iter()
                    .any(|name| radios.lines().any(|line| line == format!("Wiphy {name}")))
                {
                    break;
                }
                if Instant::now() > deadline {
                    return Err("Owned hwsim radios still exist after closing their lease; inspect lab recovery".into());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        self.owner.take();
        self.station.take();
        remove_owned_directory(&self.directory)?;
        self.cleaned = true;
        Ok(())
    }
}
impl Drop for Resources {
    fn drop(&mut self) {
        if !self.cleaned {
            // Preserve actionable diagnostics in the GUI before removing the
            // private runtime. Never claim users can inspect a deleted log.
            use std::io::{Seek, SeekFrom};
            for name in ["hostapd.log", "supplicant.log"] {
                if let Ok(mut file) = OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_NOFOLLOW)
                    .open(self.directory.join(name))
                {
                    if let Ok(meta) = file.metadata() {
                        if meta.is_file() && meta.uid() == 0 && meta.nlink() == 1 {
                            let _ = file.seek(SeekFrom::Start(meta.len().saturating_sub(4096)));
                            let mut bytes = Vec::new();
                            if file.take(4096).read_to_end(&mut bytes).is_ok() && !bytes.is_empty()
                            {
                                let text: String = String::from_utf8_lossy(&bytes)
                                    .chars()
                                    .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
                                    .rev()
                                    .take(1500)
                                    .collect::<Vec<_>>()
                                    .into_iter()
                                    .rev()
                                    .collect();
                                eprintln!("{name}:\n{text}");
                            }
                        }
                    }
                }
            }
        }
        if let Err(error) = self.cleanup() {
            eprintln!("Virtual lab cleanup required: {error}");
        }
    }
}

fn validate_root() -> Result<PathBuf, String> {
    let root = PathBuf::from(ROOT);
    if !root.exists() {
        fs::create_dir(&root).map_err(|e| e.to_string())?;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    }
    let meta = fs::symlink_metadata(&root).map_err(|e| e.to_string())?;
    if !meta.is_dir() || meta.uid() != 0 || meta.mode() & 0o077 != 0 {
        return Err("Unsafe virtual lab runtime directory".into());
    }
    let file = File::open(&root).map_err(|e| e.to_string())?;
    let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstatfs(file.as_raw_fd(), &mut stat) } != 0
        || stat.f_type != libc::TMPFS_MAGIC
    {
        return Err("Virtual lab runtime requires /run on tmpfs; no disk fallback".into());
    }
    Ok(root)
}
fn remove_owned_directory(directory: &Path) -> Result<(), String> {
    fn inspect(
        path: &Path,
        depth: usize,
        files: &mut Vec<PathBuf>,
        dirs: &mut Vec<PathBuf>,
    ) -> Result<(), String> {
        if depth > 2 || files.len() > 64 {
            return Err("Unexpected virtual lab runtime contents".into());
        }
        let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if meta.uid() != 0 || !meta.is_dir() || meta.mode() & 0o077 != 0 {
            return Err("Unexpected virtual lab directory identity".into());
        }
        for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let meta = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
            if meta.uid() != 0 {
                return Err("Foreign file in virtual lab runtime".into());
            }
            if meta.is_dir() {
                if depth != 0 || !matches!(name.as_str(), "capture" | "hostapd" | "supplicant") {
                    return Err("Unknown virtual lab subdirectory".into());
                }
                inspect(&entry.path(), depth + 1, files, dirs)?;
            } else {
                let known = if depth == 0 {
                    matches!(
                        name.as_str(),
                        "manifest.json"
                            | "hostapd.conf"
                            | "supplicant.conf"
                            | "hostapd.log"
                            | "supplicant.log"
                    ) || name.starts_with("query-") && name.ends_with(".sock")
                } else {
                    matches!(name.as_str(), "capture-01.cap" | "daap0" | "dasta0")
                };
                use std::os::unix::fs::FileTypeExt;
                if !known || !(meta.file_type().is_socket() || meta.is_file() && meta.nlink() == 1)
                {
                    return Err(format!("Unknown or linked virtual lab file: {name}"));
                }
                files.push(entry.path());
            }
        }
        dirs.push(path.into());
        Ok(())
    }
    let mut files = vec![];
    let mut dirs = vec![];
    inspect(directory, 0, &mut files, &mut dirs)?;
    files.sort_by_key(|p| p.file_name().is_some_and(|n| n == "manifest.json"));
    for file in files {
        fs::remove_file(file).map_err(|e| e.to_string())?;
    }
    for dir in dirs {
        fs::remove_dir(dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub fn recover(uid: u32) -> Result<usize, String> {
    let root = validate_root()?;
    let mut recovered = 0;
    for entry in fs::read_dir(&root).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("Invalid lab runtime name")?;
        let token = uuid::Uuid::parse_str(name).map_err(|_| "Unknown virtual lab runtime entry")?;
        let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if !meta.is_dir() || meta.uid() != 0 {
            return Err("Unexpected lab recovery directory".into());
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path.join("manifest.json"))
            .map_err(|e| e.to_string())?;
        let record: Journal =
            serde_json::from_reader(file.take(8192)).map_err(|e| e.to_string())?;
        if record.uid != uid {
            continue;
        }
        if record.token != token.to_string() {
            return Err("Lab recovery identity mismatch".into());
        }
        record.validate()?;
        if !record.start.is_empty() && proc_start(record.pid) == record.start {
            return Err("Virtual lab worker is still active; stop it first".into());
        }
        for child in &record.children {
            if !child.start.is_empty() && proc_start(child.pid) == child.start {
                unsafe {
                    libc::kill(child.pid as i32, libc::SIGTERM);
                }
                let deadline = Instant::now() + Duration::from_secs(2);
                while proc_start(child.pid) == child.start && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(30));
                }
                if proc_start(child.pid) == child.start {
                    return Err("An owned lab process is still stopping; retry recovery".into());
                }
            }
        }
        remove_owned_directory(&path)?;
        recovered += 1;
    }
    Ok(recovered)
}

struct ControlPath(PathBuf);
impl Drop for ControlPath {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn query(directory: &Path, endpoint: &str, command: &str) -> Result<String, String> {
    let path = directory.join(format!(
        "query-{}.sock",
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    ));
    let socket = UnixDatagram::bind(&path).map_err(|e| e.to_string())?;
    let _cleanup = ControlPath(path);
    socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    socket
        .connect(directory.join(endpoint))
        .map_err(|e| e.to_string())?;
    socket.send(command.as_bytes()).map_err(|e| e.to_string())?;
    let mut bytes = [0; 8192];
    let read = socket.recv(&mut bytes).map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&bytes[..read]).into_owned())
}
fn value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|line| {
        line.split_once('=')
            .filter(|(name, _)| *name == key)
            .map(|(_, value)| value)
    })
}
fn log(status: &mut VirtualStatus, message: String) {
    if status.logs.len() >= 60 {
        status.logs.remove(0);
    }
    status
        .logs
        .push(format!("[{:03}s] {message}", status.elapsed_seconds));
}
fn emit(status: &VirtualStatus) -> Result<(), String> {
    send(serde_json::json!({"kind":"lab-state","status":status}))
}

pub fn run_lab(uid: u32, lab_id: &str, token: &str) -> Result<(), String> {
    if lab_id != LAB_ID {
        return Err("Unknown built-in virtual lab".into());
    }
    // A full pipe must respect send()'s deadline rather than blocking cleanup.
    let flags = unsafe { libc::fcntl(1, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(1, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io_error("Configure bounded lab output"));
    }
    let token = uuid::Uuid::parse_str(token).map_err(|_| "Invalid virtual lab token")?;
    let root = validate_root()?;
    let directory = root.join(token.to_string());
    fs::create_dir(&directory).map_err(|e| e.to_string())?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
        .map_err(|e| e.to_string())?;
    let mut resources = Resources {
        directory: directory.clone(),
        journal: Journal {
            uid,
            pid: std::process::id(),
            start: proc_start(std::process::id()),
            token: token.to_string(),
            children: vec![],
        },
        children: vec![],
        collector: None,
        radio_socket: None,
        owner: None,
        station: None,
        radio_names: vec![],
        cleaned: false,
    };
    resources.save()?;
    let stopped = Arc::new(AtomicBool::new(false));
    for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
        signal_hook::flag::register(signal, stopped.clone()).map_err(|e| e.to_string())?;
    }
    if unsafe { libc::unshare(libc::CLONE_NEWNET) } != 0 {
        return Err(io_error(
            "Create isolated virtual WLAN namespace (requires CAP_SYS_ADMIN)",
        ));
    }
    resources.owner = Some(File::open("/proc/self/ns/net").map_err(|e| e.to_string())?);
    if unsafe { libc::unshare(libc::CLONE_NEWNET) } != 0 {
        return Err(io_error("Create isolated client namespace"));
    }
    resources.station = Some(File::open("/proc/self/ns/net").map_err(|e| e.to_string())?);
    let owner = resources.owner.as_ref().unwrap().as_raw_fd();
    let station = resources.station.as_ref().unwrap().as_raw_fd();
    enter_namespace(owner)?;
    if !Path::new("/sys/module/mac80211_hwsim").is_dir() {
        run(owner, "modprobe", &["mac80211_hwsim", "radios=0"])?;
    }
    let mut radios = GenericNetlink::open()?;
    let hwsim = radios.family("MAC80211_HWSIM")?;
    // All radios originate in the same private hwsim netgroup. Moving the
    // client wiphy preserves that group while separating its IP stack.
    let mut nl = GenericNetlink::open()?;
    let nl80211 = nl.family("nl80211")?;
    let bytes = token.as_bytes();
    let base = [2, 0xda, bytes[0], bytes[1], bytes[2], 0];
    let mut ap_mac = base;
    ap_mac[5] = 1;
    let mut sta_mac = base;
    sta_mac[5] = 2;
    let mut mon_mac = base;
    mon_mac[5] = 3;
    let token_short = token.simple().to_string();
    let suffix = &token_short[..8];
    let names = [
        format!("da{suffix}a"),
        format!("da{suffix}s"),
        format!("da{suffix}m"),
    ];
    resources.radio_socket = Some(radios);
    resources.radio_names = names.to_vec();
    for (name, mac) in names.iter().zip([ap_mac, sta_mac, mon_mac]) {
        resources
            .radio_socket
            .as_mut()
            .unwrap()
            .new_radio(hwsim, name, mac)?;
    }
    let sta_index = nl.wiphy_index(nl80211, &names[1])?;
    let ap_identifier = format!("phy#{}", nl.wiphy_index(nl80211, &names[0])?);
    let monitor_identifier = format!("phy#{}", nl.wiphy_index(nl80211, &names[2])?);
    let station_identifier = format!("phy#{sta_index}");
    nl.move_wiphy(nl80211, sta_index, station)?;
    run(
        owner,
        "iw",
        &[&ap_identifier, "interface", "add", AP, "type", "managed"],
    )?;
    run(
        station,
        "iw",
        &[
            &station_identifier,
            "interface",
            "add",
            STA,
            "type",
            "managed",
        ],
    )?;
    run(
        owner,
        "iw",
        &[
            &monitor_identifier,
            "interface",
            "add",
            MON,
            "type",
            "monitor",
        ],
    )?;
    for namespace in [owner, station] {
        run(namespace, "ip", &["link", "set", "dev", "lo", "up"])?;
    }
    run(owner, "ip", &["addr", "add", "10.88.0.1/24", "dev", AP])?;
    run(station, "ip", &["addr", "add", "10.88.0.2/24", "dev", STA])?;
    run(station, "ip", &["link", "set", "dev", STA, "up"])?;
    run(owner, "ip", &["link", "set", "dev", MON, "up"])?;
    run(owner, "iw", &["dev", MON, "set", "freq", "2437"])?;
    for name in ["hostapd", "supplicant"] {
        fs::create_dir(directory.join(name)).map_err(|e| e.to_string())?;
        fs::set_permissions(directory.join(name), fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
    }
    let hostapd=format!("interface={AP}\ndriver=nl80211\nctrl_interface={}/hostapd\nssid={LAB_SSID}\nhw_mode=g\nchannel=6\nwpa=2\nwpa_key_mgmt=WPA-PSK\nrsn_pairwise=CCMP\nwpa_passphrase={LAB_PASSPHRASE}\nieee80211w=1\n",directory.display());
    write_private(&directory.join("hostapd.conf"), hostapd.as_bytes())?;
    let bssid = ap_mac
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":");
    let supplicant=format!("ctrl_interface={}/supplicant\nupdate_config=0\np2p_disabled=1\nnetwork={{\n ssid=\"{LAB_SSID}\"\n bssid={bssid}\n psk=\"{LAB_PASSPHRASE}\"\n key_mgmt=WPA-PSK\n pairwise=CCMP\n group=CCMP\n ieee80211w=1\n scan_freq=2437\n}}\n",directory.display());
    write_private(&directory.join("supplicant.conf"), supplicant.as_bytes())?;
    resources.spawn(
        owner,
        "hostapd",
        &[directory
            .join("hostapd.conf")
            .to_str()
            .ok_or("Invalid runtime path")?],
        "hostapd.log",
    )?;
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        if let Ok(reply) = query(&directory, &format!("hostapd/{AP}"), "STATUS") {
            if value(&reply, "state") == Some("ENABLED") {
                break;
            }
        }
        if Instant::now() > deadline {
            return Err("Virtual AP did not reach ENABLED; inspect the owned hostapd log".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    resources.collector = Some(crate::aircrack::linux::Airodump::start(
        directory.join("capture"),
        MON,
        &bssid,
        2437,
    )?);
    resources.track(resources.collector.as_ref().unwrap().pid())?;
    let mut status = VirtualStatus {
        running: true,
        phase: "AP READY / CLIENT IDLE".into(),
        operation_id: token.to_string(),
        ssid: LAB_SSID.into(),
        bssid: bssid.clone(),
        ap_ready: true,
        limit_seconds: LIMIT_SECONDS,
        ..Default::default()
    };
    log(&mut status,"Three owned hwsim radios created in an isolated virtual medium; no physical radio selected.".into());
    log(
        &mut status,
        "hostapd reports ENABLED. Observer is recording channel 6.".into(),
    );
    emit(&status)?;
    let start = Instant::now();
    let mut report_at = Instant::now();
    let mut tracker = crate::packets::Tracker::default();
    let mut connected = false;
    let mut command_bytes = vec![];
    let mut client_started = false;
    while !stopped.load(Ordering::Relaxed) && start.elapsed() < Duration::from_secs(LIMIT_SECONDS) {
        status.elapsed_seconds = start.elapsed().as_secs();
        for _ in 0..64 {
            match resources.collector.as_mut().unwrap().next()? {
                Some(frame) => {
                    if let Some((_, key)) = crate::packets::parse(&frame.data, frame.time, ap_mac) {
                        if let Some(key) = key {
                            tracker.observe(key);
                        }
                        status.packet_count += 1;
                        send(
                            serde_json::json!({"kind":"packet","time":frame.time,"data":frame.data}),
                        )?;
                    }
                }
                None => break,
            }
        }
        let exchanges = tracker.exchanges();
        status.eapol_messages = exchanges
            .iter()
            .flat_map(|e| e.messages.iter().copied())
            .collect();
        status.eapol_messages.sort();
        status.eapol_messages.dedup();
        status.exchange_correlated = exchanges.iter().any(|e| e.complete);
        let mut poll = libc::pollfd {
            fd: 0,
            events: libc::POLLIN | libc::POLLHUP,
            revents: 0,
        };
        unsafe {
            libc::poll(&mut poll, 1, 20);
        }
        if poll.revents & (libc::POLLHUP | libc::POLLERR) != 0 {
            break;
        }
        if poll.revents & libc::POLLIN != 0 {
            let mut bytes = [0; 64];
            let n = unsafe { libc::read(0, bytes.as_mut_ptr().cast(), bytes.len()) };
            if n <= 0 {
                break;
            }
            command_bytes.extend(&bytes[..n as usize]);
            if command_bytes.len() > 64 {
                return Err("Virtual lab action exceeds bounds".into());
            }
            if let Some(end) = command_bytes.iter().position(|b| *b == b'\n') {
                let command = String::from_utf8(command_bytes.drain(..=end).collect())
                    .map_err(|_| "Invalid virtual lab action")?;
                let command = command.trim();
                if command == "stop" {
                    break;
                }
                status.busy = true;
                status.error = None;
                status.phase = format!("RUNNING {}", command.to_uppercase());
                emit(&status)?;
                let action = (|| -> Result<(), String> {
                    match command {
                        "scan" => {
                            if client_started {
                                return Err(
                                    "Scan before starting the client; reset the lab to repeat"
                                        .into(),
                                );
                            }
                            let scan =
                                output(station, "iw", &["dev", STA, "scan", "freq", "2437"], 12)?;
                            status.scan_ssids = scan
                                .lines()
                                .filter_map(|line| line.trim().strip_prefix("SSID: "))
                                .take(16)
                                .map(|ssid| ssid.chars().take(32).collect())
                                .collect();
                            status.scan_confirmed =
                                scan.to_ascii_uppercase().contains(&format!("BSS {bssid}"));
                            if !status.scan_confirmed {
                                return Err(
                                    "The owned AP was not found by the actual station scan".into(),
                                );
                            }
                            let scan_count = status.scan_ssids.len();
                            log(&mut status,format!("Station scan found the owned AP; {scan_count} SSID(s) visible in this virtual medium."));
                        }
                        "connect" => {
                            if !status.scan_confirmed {
                                return Err("Run the station scan first".into());
                            }
                            if status.packet_count == 0 {
                                return Err("Wait for the observer to receive actual AP frames before connecting".into());
                            }
                            if client_started {
                                return Err("The client is already started".into());
                            }
                            resources.spawn(
                                station,
                                "wpa_supplicant",
                                &[
                                    "-Dnl80211",
                                    "-i",
                                    STA,
                                    "-c",
                                    directory.join("supplicant.conf").to_str().unwrap(),
                                ],
                                "supplicant.log",
                            )?;
                            client_started = true;
                            status.client_started = true;
                            log(&mut status,"wpa_supplicant started on the owned client. Waiting for its actual state.".into());
                        }
                        "verify" => {
                            if !status.client_connected || !status.ap_authorized {
                                return Err(
                                    "Wait for COMPLETED on the client and AUTHORIZED on the AP"
                                        .into(),
                                );
                            }
                            let result = output(
                                station,
                                "ping",
                                &["-n", "-I", STA, "-c", "1", "-W", "2", AP_ADDRESS],
                                5,
                            )?;
                            status.ping_verified = true;
                            log(
                                &mut status,
                                format!(
                                    "Client-to-AP ICMP succeeded through the virtual WLAN: {}",
                                    result
                                        .lines()
                                        .find(|line| line.contains("received"))
                                        .unwrap_or("ping exit status 0")
                                ),
                            );
                        }
                        _ => return Err("Unsupported virtual lab action".into()),
                    }
                    Ok(())
                })();
                if let Err(error) = action {
                    log(&mut status, error.clone());
                    status.error = Some(error);
                }
                status.busy = false;
                status.phase = if client_started {
                    "CLIENT CONNECTING / OBSERVING"
                } else {
                    "AP READY / CLIENT IDLE"
                }
                .into();
                emit(&status)?;
            }
        }
        if report_at.elapsed() >= Duration::from_secs(1) {
            for child in &mut resources.children {
                if let Some(exit) = child.try_wait().map_err(|e| e.to_string())? {
                    return Err(format!(
                        "Virtual lab service exited unexpectedly: {exit}; inspect its owned log"
                    ));
                }
            }
            let reply = query(&directory, &format!("hostapd/{AP}"), "STATUS")?;
            status.ap_ready = value(&reply, "state") == Some("ENABLED");
            if client_started {
                let reply = query(&directory, &format!("supplicant/{STA}"), "STATUS");
                status.client_connected = reply
                    .as_ref()
                    .is_ok_and(|text| value(text, "wpa_state") == Some("COMPLETED"));
                let station_mac = sta_mac
                    .iter()
                    .map(|b| format!("{b:02X}"))
                    .collect::<Vec<_>>()
                    .join(":");
                let station_reply = query(
                    &directory,
                    &format!("hostapd/{AP}"),
                    &format!("STA {station_mac}"),
                );
                status.ap_authorized = station_reply
                    .as_ref()
                    .is_ok_and(|text| text.contains("[AUTHORIZED]"));
                if status.client_connected && !connected {
                    log(&mut status,"Client reports COMPLETED; AP station authorization is checked independently.".into());
                }
                connected = status.client_connected;
            }
            let was_complete = status.completed;
            status.completed = status.has_completion_evidence();
            if status.completed && !was_complete {
                status.phase = "LAB VERIFIED".into();
                log(&mut status,"Verified: scan, WPA2 association, AP authorization, M1–M4 correlation and real IP connectivity.".into());
            }
            if status.client_connected && !status.completed {
                status.phase = "CLIENT ASSOCIATED / VERIFY CONNECTIVITY".into();
            }
            for name in ["hostapd.log", "supplicant.log"] {
                if fs::metadata(directory.join(name)).is_ok_and(|m| m.len() > 2 * 1024 * 1024) {
                    return Err("Virtual lab service log limit reached".into());
                }
            }
            emit(&status)?;
            report_at = Instant::now();
        }
    }
    status.busy = true;
    status.phase = "STOPPING OWNED LAB".into();
    emit(&status)?;
    resources.cleanup()?;
    status.running = false;
    status.busy = false;
    status.cleanup_complete = true;
    status.phase = "STOPPED / OWNED RESOURCES REMOVED".into();
    status.error = None;
    emit(&status)?;
    send(serde_json::json!({"kind":"lab-done","clean":true}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_rejects_process_groups_and_unbounded_process_lists() {
        let mut journal = Journal {
            uid: 1000,
            pid: 2,
            start: "123".into(),
            token: uuid::Uuid::new_v4().to_string(),
            children: vec![],
        };
        assert!(journal.validate().is_ok());
        for pid in [0, 1, u32::MAX] {
            journal.pid = pid;
            assert!(journal.validate().is_err());
        }
        journal.pid = 2;
        journal.children.push(ProcessRecord {
            pid: 0,
            start: "1".into(),
        });
        assert!(journal.validate().is_err());
        journal.children.clear();
        journal.start.clear();
        assert!(journal.validate().is_err());
    }
    #[test]
    fn cleanup_refuses_unknown_files_before_removing_any_owned_file() {
        if unsafe { libc::geteuid() } != 0 {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(dir.path().join("manifest.json"), b"owned").unwrap();
        fs::write(dir.path().join("foreign"), b"keep").unwrap();
        assert!(remove_owned_directory(dir.path()).is_err());
        assert!(dir.path().join("manifest.json").exists());
        assert!(dir.path().join("foreign").exists());
    }
}
