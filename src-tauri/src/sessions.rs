use crate::model::{Policy, Session};
#[cfg(target_os = "linux")]
use crate::storage::Dir;
use std::{fs::File, path::PathBuf};

pub struct Sessions {
    root: PathBuf,
    disk_root: PathBuf,
    ram_root: Option<PathBuf>,
    pub current: Option<Session>,
    lock: Option<File>,
}
impl Sessions {
    pub fn new(root: PathBuf) -> Self {
        Self {
            disk_root: root.clone(),
            root,
            ram_root: runtime_root().ok(),
            current: None,
            lock: None,
        }
    }
    pub fn active(&self) -> Result<&Session, String> {
        self.current
            .as_ref()
            .filter(|s| s.state == "ACTIVE")
            .ok_or_else(|| "Start an active session first".into())
    }
    pub fn disk_path(&self) -> Result<PathBuf, String> {
        self.active()?
            .path
            .as_ref()
            .map(PathBuf::from)
            .ok_or_else(|| {
                "This operation writes files; start a temporary or persistent session".into()
            })
    }
    #[cfg(target_os = "linux")]
    pub fn start(&mut self, policy: Policy) -> Result<Session, String> {
        use fs2::FileExt;
        if self.current.as_ref().is_some_and(|s| s.state != "CLOSED") {
            return Err("End or recover the current session first".into());
        }
        self.root = if policy == Policy::RamOnly {
            self.ram_root.clone().ok_or("RAM session unavailable: XDG_RUNTIME_DIR must be private, user-owned and backed by tmpfs. No disk fallback.")?
        } else {
            self.disk_root.clone()
        };
        let id = uuid::Uuid::new_v4().to_string();
        let mut session = Session {
            id: id.clone(),
            policy: policy.clone(),
            state: "PREPARING".into(),
            path: None,
            files: vec![],
        };
        if policy != Policy::MemoryOnly {
            let root = Dir::root(&self.root)?;
            let dir = root.child(&id, true)?;
            let lock = dir.file(".lock", true)?;
            lock.try_lock_exclusive().map_err(|e| e.to_string())?;
            self.lock = Some(lock);
            session.path = Some(self.root.join(&id).to_string_lossy().into_owned());
            session.files = vec![".lock".into(), "manifest.json".into()];
            dir.replace_manifest(&serde_json::to_vec(&session).map_err(|e| e.to_string())?)?;
        }
        session.state = "ACTIVE".into();
        self.current = Some(session);
        self.persist()?;
        self.status()
    }
    #[cfg(not(target_os = "linux"))]
    pub fn start(&mut self, _: Policy) -> Result<Session, String> {
        Err("Sessions require the Linux desktop runtime".into())
    }
    #[cfg(target_os = "linux")]
    fn dir(&self) -> Result<Dir, String> {
        let session = self.active()?;
        if session.policy == Policy::MemoryOnly {
            return Err("Memory-only session has no files".into());
        }
        Dir::root(&self.root)?.child(&session.id, false)
    }
    #[cfg(target_os = "linux")]
    fn persist(&self) -> Result<(), String> {
        if let Some(s) = &self.current {
            if s.policy != Policy::MemoryOnly {
                Dir::root(&self.root)?
                    .child(&s.id, false)?
                    .replace_manifest(&serde_json::to_vec(s).map_err(|e| e.to_string())?)?
            }
        }
        Ok(())
    }
    #[cfg(target_os = "linux")]
    pub fn status(&self) -> Result<Session, String> {
        let mut s = self.current.clone().ok_or("No session")?;
        if s.policy != Policy::MemoryOnly && s.state != "CLOSED" {
            s.files = Dir::root(&self.root)?.child(&s.id, false)?.inventory("")?
        }
        Ok(s)
    }
    #[cfg(not(target_os = "linux"))]
    pub fn status(&self) -> Result<Session, String> {
        self.current.clone().ok_or("No session".into())
    }
    #[cfg(target_os = "linux")]
    pub fn write(&mut self, kind: &str, extension: &str, data: &[u8]) -> Result<String, String> {
        self.disk_path()?;
        if !matches!(
            (kind, extension),
            ("report", "json") | ("capture", "pcapng")
        ) {
            return Err("Unsupported export type".into());
        }
        let name = format!("{kind}-{}.{}", uuid::Uuid::new_v4(), extension);
        self.current
            .as_mut()
            .ok_or("No session")?
            .files
            .push(name.clone());
        self.persist()?;
        self.dir()?.write(&name, data)?;
        Ok(self.disk_path()?.join(name).to_string_lossy().into_owned())
    }
    #[cfg(not(target_os = "linux"))]
    pub fn write(&mut self, _: &str, _: &str, _: &[u8]) -> Result<String, String> {
        Err("Linux required".into())
    }
    #[cfg(target_os = "linux")]
    pub fn lab_dir(&mut self) -> Result<PathBuf, String> {
        let path = self.disk_path()?;
        let name = format!("lab-{}", uuid::Uuid::new_v4());
        self.current
            .as_mut()
            .ok_or("No session")?
            .files
            .push(format!("{name}/"));
        self.persist()?;
        self.dir()?.child(&name, true)?;
        Ok(path.join(name))
    }
    #[cfg(not(target_os = "linux"))]
    pub fn lab_dir(&mut self) -> Result<PathBuf, String> {
        Err("Linux required".into())
    }
    #[cfg(target_os = "linux")]
    pub fn end(&mut self) -> Result<Session, String> {
        let mut s = self.current.clone().ok_or("No session")?;
        if s.state == "CLOSED" {
            return Ok(s);
        }
        self.current.as_mut().unwrap().state = "CLOSING".into();
        self.persist()?;
        if matches!(s.policy, Policy::Temporary | Policy::RamOnly) {
            let result = self.purge_owned(&s);
            if let Err(e) = result {
                self.current.as_mut().unwrap().state = "FAILED".into();
                self.persist()?;
                return Err(e);
            }
            s.files.clear();
        }
        self.lock = None;
        s.state = "CLOSED".into();
        self.current = Some(s.clone());
        if s.policy == Policy::Persistent {
            self.persist()?
        }
        Ok(s)
    }
    #[cfg(not(target_os = "linux"))]
    pub fn end(&mut self) -> Result<Session, String> {
        Err("Linux required".into())
    }
    #[cfg(target_os = "linux")]
    fn purge_owned(&self, s: &Session) -> Result<(), String> {
        let root = Dir::root(if s.policy == Policy::RamOnly {
            self.ram_root.as_ref().ok_or("RAM storage unavailable")?
        } else {
            &self.disk_root
        })?;
        let dir = root.child(&s.id, false)?;
        let manifest: Session = serde_json::from_slice(&dir.read("manifest.json", 1048576)?)
            .map_err(|e| e.to_string())?;
        if manifest.id != s.id || !matches!(manifest.policy, Policy::Temporary | Policy::RamOnly) {
            return Err("Refusing unowned or persistent directory".into());
        }
        for name in dir.entries()? {
            let permitted = manifest.files.contains(&name)
                || manifest.files.contains(&format!("{name}/"))
                || name.starts_with("manifest-") && name.ends_with(".next");
            if !permitted {
                return Err(format!("Unlisted file retained: {name}"));
            }
        }
        dir.purge_tree()?;
        root.remove_dir(&s.id)
    }
    #[cfg(target_os = "linux")]
    pub fn recover(&self) -> Result<Vec<Session>, String> {
        use fs2::FileExt;
        let mut sessions = Vec::new();
        for path in std::iter::once(&self.disk_root).chain(self.ram_root.iter()) {
            if !path.exists() {
                continue;
            }
            let root = Dir::root(path)?;
            for id in root.entries()? {
                if uuid::Uuid::parse_str(&id).is_err() {
                    continue;
                }
                if self.current.as_ref().is_some_and(|s| s.id == id) {
                    continue;
                }
                let dir = root.child(&id, false)?;
                let lock = dir.file(".lock", false)?;
                if lock.try_lock_exclusive().is_err() {
                    continue;
                }
                let mut s: Session = serde_json::from_slice(&dir.read("manifest.json", 1048576)?)
                    .map_err(|e| e.to_string())?;
                if s.id != id || !matches!(s.policy, Policy::Temporary | Policy::RamOnly) {
                    continue;
                }
                s.state = "STALE".into();
                s.path = Some(path.join(&id).to_string_lossy().into_owned());
                s.files = dir.inventory("")?;
                sessions.push(s);
            }
        }
        Ok(sessions)
    }
    #[cfg(not(target_os = "linux"))]
    pub fn recover(&self) -> Result<Vec<Session>, String> {
        Err("Linux required".into())
    }
    #[cfg(target_os = "linux")]
    pub fn purge(&self, id: &str) -> Result<(), String> {
        use fs2::FileExt;
        if uuid::Uuid::parse_str(id).is_err() {
            return Err("Invalid session identifier".into());
        }
        let s = self
            .recover()?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or("Stale session not found")?;
        let dir = Dir::root(if s.policy == Policy::RamOnly {
            self.ram_root.as_ref().ok_or("RAM storage unavailable")?
        } else {
            &self.disk_root
        })?
        .child(id, false)?;
        let lock = dir.file(".lock", false)?;
        lock.try_lock_exclusive().map_err(|_| "Session is active")?;
        self.purge_owned(&s)
    }
    #[cfg(not(target_os = "linux"))]
    pub fn purge(&self, _: &str) -> Result<(), String> {
        Err("Linux required".into())
    }
    /// Explicit PANIC scope: only validated app-owned session UUID directories.
    #[cfg(target_os = "linux")]
    pub fn panic_purge(&mut self, all: bool, shred: bool) -> Result<usize, String> {
        use fs2::FileExt;
        if shred {
            crate::process::executable("shred")?;
        }
        let mut plan = Vec::new();
        for path in std::iter::once(&self.disk_root).chain(self.ram_root.iter()) {
            if !path.exists() {
                continue;
            }
            let root = Dir::root(path)?;
            for id in root.entries()? {
                if !all && self.current.as_ref().is_none_or(|s| s.id != id) {
                    continue;
                }
                if uuid::Uuid::parse_str(&id).is_err() {
                    return Err("Unknown entry in session root; PANIC cleanup refused".into());
                }
                let dir = root.child(&id, false)?;
                let held = self.current.as_ref().is_some_and(|s| s.id == id) && self.lock.is_some();
                let lock = if held {
                    None
                } else {
                    let file = dir.file(".lock", false)?;
                    file.try_lock_exclusive().map_err(|_| {
                        "Another application instance owns a session; close it before PANIC cleanup"
                    })?;
                    Some(file)
                };
                let manifest: Session =
                    serde_json::from_slice(&dir.read("manifest.json", 1048576)?)
                        .map_err(|e| e.to_string())?;
                if manifest.id != id || manifest.policy == Policy::MemoryOnly {
                    return Err("Invalid session manifest; cleanup refused".into());
                }
                for name in dir.entries()? {
                    if !manifest.files.contains(&name)
                        && !manifest.files.contains(&format!("{name}/"))
                        && !(name.starts_with("manifest-") && name.ends_with(".next"))
                    {
                        return Err(format!("Unlisted file retained: {name}"));
                    }
                }
                dir.inventory("")?;
                plan.push((path.clone(), id, dir, lock));
            }
        }
        // Complete preflight of every selected session before deleting any data.
        let count = plan.len();
        for (path, id, dir, _lock) in plan {
            dir.purge_tree_with(shred)?;
            Dir::root(&path)?.remove_dir(&id)?;
            if self.current.as_ref().is_some_and(|s| s.id == id) {
                self.current = None;
                self.lock = None;
            }
        }
        self.current = None;
        self.lock = None;
        Ok(count)
    }
    #[cfg(not(target_os = "linux"))]
    pub fn panic_purge(&mut self, _: bool, _: bool) -> Result<usize, String> {
        Err("Linux required".into())
    }
}

#[cfg(target_os = "linux")]
fn runtime_root() -> Result<PathBuf, String> {
    use std::os::unix::fs::MetadataExt;
    let path = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", unsafe { libc::geteuid() })));
    let meta = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    if !path.is_absolute()
        || !meta.is_dir()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.mode() & 0o077 != 0
    {
        return Err("Unsafe runtime directory".into());
    }
    let name =
        std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).map_err(|e| e.to_string())?;
    let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(name.as_ptr(), &mut stat) } != 0 || stat.f_type != libc::TMPFS_MAGIC {
        return Err("Runtime directory is not tmpfs".into());
    }
    Ok(path.join("deadair-sessions"))
}
#[cfg(not(target_os = "linux"))]
fn runtime_root() -> Result<PathBuf, String> {
    Err("Linux required".into())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    #[test]
    fn panic_erases_persistent_and_temporary_but_not_external_files() {
        let temp = tempfile::tempdir().unwrap();
        let outside = temp.path().join("keep");
        std::fs::write(&outside, b"keep").unwrap();
        let mut sessions = Sessions::new(temp.path().join("sessions"));
        sessions.start(Policy::Persistent).unwrap();
        sessions
            .write("capture", "pcapng", b"test capture")
            .unwrap();
        sessions.end().unwrap();
        sessions.start(Policy::Temporary).unwrap();
        sessions.write("report", "json", b"{}").unwrap();
        let lab = sessions.lab_dir().unwrap();
        std::fs::write(lab.join("test.txt"), b"test vector").unwrap();
        assert_eq!(sessions.panic_purge(true, true).unwrap(), 2);
        assert_eq!(std::fs::read(&outside).unwrap(), b"keep");
        assert!(sessions.current.is_none());
    }
    #[test]
    fn panic_current_scope_preserves_other_saved_sessions() {
        let temp = tempfile::tempdir().unwrap();
        let mut sessions = Sessions::new(temp.path().join("sessions"));
        let saved = sessions.start(Policy::Persistent).unwrap();
        sessions.end().unwrap();
        sessions.start(Policy::Temporary).unwrap();
        assert_eq!(sessions.panic_purge(false, false).unwrap(), 1);
        assert!(PathBuf::from(saved.path.unwrap()).exists());
    }
    #[test]
    fn panic_preflight_rejects_link_before_overwriting_payload() {
        let temp = tempfile::tempdir().unwrap();
        let mut sessions = Sessions::new(temp.path().join("sessions"));
        sessions.start(Policy::Persistent).unwrap();
        let file = sessions.write("report", "json", b"preserve").unwrap();
        let lab = sessions.lab_dir().unwrap();
        std::os::unix::fs::symlink(&file, lab.join("link")).unwrap();
        assert!(sessions.panic_purge(true, true).is_err());
        assert_eq!(std::fs::read(file).unwrap(), b"preserve");
    }
    #[test]
    fn panic_refuses_a_session_locked_by_another_instance() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("sessions");
        let mut first = Sessions::new(root.clone());
        first.start(Policy::Temporary).unwrap();
        let file = first.write("report", "json", b"preserve").unwrap();
        let mut second = Sessions::new(root);
        assert!(second.panic_purge(true, true).is_err());
        assert_eq!(std::fs::read(file).unwrap(), b"preserve");
    }
    #[test]
    fn ram_session_keeps_lab_and_exports_off_disk_root() {
        let temp = tempfile::tempdir().unwrap();
        let ram = tempfile::tempdir_in("/dev/shm").unwrap();
        let disk = temp.path().join("disk");
        let mut sessions = Sessions::new(disk.clone());
        sessions.ram_root = Some(ram.path().join("sessions"));
        let info = sessions.start(Policy::RamOnly).unwrap();
        assert!(PathBuf::from(info.path.unwrap()).starts_with(ram.path()));
        sessions.write("report", "json", b"{}").unwrap();
        assert!(sessions.lab_dir().unwrap().starts_with(ram.path()));
        assert!(!disk.exists());
        sessions.end().unwrap();
        assert!(!disk.exists());
    }
    #[test]
    fn memory_creates_no_directory() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("absent");
        let mut s = Sessions::new(root.clone());
        s.start(Policy::MemoryOnly).unwrap();
        assert!(!root.exists());
        assert!(s.disk_path().is_err());
        s.end().unwrap();
        assert!(!root.exists());
    }
    #[test]
    fn temporary_removes_only_manifest_owned_data() {
        let t = tempfile::tempdir().unwrap();
        let mut s = Sessions::new(t.path().join("sessions"));
        let info = s.start(Policy::Temporary).unwrap();
        let path = s.write("report", "json", b"{}").unwrap();
        assert!(std::path::Path::new(&path).exists());
        s.end().unwrap();
        assert!(!std::path::Path::new(info.path.as_ref().unwrap()).exists());
    }
    #[test]
    fn stale_and_persistent_are_distinct() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("sessions");
        let id;
        {
            let mut s = Sessions::new(root.clone());
            id = s.start(Policy::Temporary).unwrap().id;
        }
        let mut s = Sessions::new(root);
        assert_eq!(s.recover().unwrap().len(), 1);
        s.purge(&id).unwrap();
        let persistent = s.start(Policy::Persistent).unwrap();
        s.end().unwrap();
        assert!(std::path::Path::new(persistent.path.as_ref().unwrap()).exists());
        assert!(s.recover().unwrap().is_empty());
    }
    #[test]
    fn symlink_cannot_escape_cleanup() {
        use std::os::unix::fs::symlink;
        let t = tempfile::tempdir().unwrap();
        let outside = t.path().join("keep");
        std::fs::write(&outside, b"keep").unwrap();
        let mut s = Sessions::new(t.path().join("sessions"));
        let session = s.start(Policy::Temporary).unwrap();
        symlink(
            &outside,
            PathBuf::from(session.path.unwrap()).join("intruder"),
        )
        .unwrap();
        assert!(s.end().is_err());
        assert_eq!(std::fs::read(outside).unwrap(), b"keep");
    }
}
