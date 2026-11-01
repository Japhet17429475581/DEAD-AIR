//! Linux directory-descriptor operations. No cleanup follows symlinks or accepts
//! client-provided paths; all names are single components under an owned root.
#[cfg(target_os = "linux")]
mod linux {
    use std::{
        ffi::CString,
        fs::File,
        io::{Read, Write},
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::fs::MetadataExt,
        },
        path::{Component, Path},
    };
    pub struct Dir {
        file: File,
    }
    fn name(s: &str) -> Result<CString, String> {
        if s.is_empty() || s == "." || s == ".." || s.contains('/') || s.contains('\\') {
            return Err("Unsafe path component".into());
        }
        CString::new(s).map_err(|_| "NUL in path".into())
    }
    fn error() -> String {
        std::io::Error::last_os_error().to_string()
    }
    impl Dir {
        pub fn root(path: &Path) -> Result<Self, String> {
            if !path.is_absolute() {
                return Err("Storage root must be absolute".into());
            }
            let file = File::open("/").map_err(|e| e.to_string())?;
            let mut dir = Self { file };
            for component in path.components() {
                match component {
                    Component::RootDir => (),
                    Component::Normal(part) => {
                        let part = part.to_str().ok_or("Non-UTF8 storage path")?;
                        dir = dir.child(part, true)?;
                    }
                    _ => return Err("Invalid storage path".into()),
                }
            }
            let m = dir.file.metadata().map_err(|e| e.to_string())?;
            if m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0 {
                return Err(
                    "Application storage root must be owned by this user with mode 0700".into(),
                );
            }
            Ok(dir)
        }
        pub fn child(&self, s: &str, create: bool) -> Result<Self, String> {
            let c = name(s)?;
            if create {
                let result = unsafe { libc::mkdirat(self.file.as_raw_fd(), c.as_ptr(), 0o700) };
                if result < 0
                    && std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST)
                {
                    return Err(error());
                }
            }
            let fd = unsafe {
                libc::openat(
                    self.file.as_raw_fd(),
                    c.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(error());
            }
            Ok(Self {
                file: unsafe { File::from_raw_fd(fd) },
            })
        }
        pub fn file(&self, s: &str, new: bool) -> Result<File, String> {
            let c = name(s)?;
            let flags = libc::O_RDWR
                | libc::O_NOFOLLOW
                | libc::O_CLOEXEC
                | if new { libc::O_CREAT | libc::O_EXCL } else { 0 };
            let fd = unsafe { libc::openat(self.file.as_raw_fd(), c.as_ptr(), flags, 0o600) };
            if fd < 0 {
                return Err(error());
            }
            let f = unsafe { File::from_raw_fd(fd) };
            let m = f.metadata().map_err(|e| e.to_string())?;
            if !m.is_file() || m.nlink() != 1 || m.uid() != unsafe { libc::geteuid() } {
                return Err("Refusing non-owned, linked, or non-regular file".into());
            }
            Ok(f)
        }
        pub fn write(&self, s: &str, data: &[u8]) -> Result<(), String> {
            let mut f = self.file(s, true)?;
            f.write_all(data).map_err(|e| e.to_string())?;
            f.sync_all().map_err(|e| e.to_string())
        }
        pub fn replace_manifest(&self, data: &[u8]) -> Result<(), String> {
            let temp = format!("manifest-{}.next", uuid::Uuid::new_v4());
            self.write(&temp, data)?;
            let old = name(&temp)?;
            let new = name("manifest.json")?;
            if unsafe {
                libc::renameat(
                    self.file.as_raw_fd(),
                    old.as_ptr(),
                    self.file.as_raw_fd(),
                    new.as_ptr(),
                )
            } < 0
            {
                return Err(error());
            }
            self.file.sync_all().map_err(|e| e.to_string())
        }
        pub fn read(&self, s: &str, limit: u64) -> Result<Vec<u8>, String> {
            let f = self.file(s, false)?;
            let mut bytes = Vec::new();
            f.take(limit + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() as u64 > limit {
                return Err("File size limit exceeded".into());
            }
            Ok(bytes)
        }
        pub fn entries(&self) -> Result<Vec<String>, String> {
            std::fs::read_dir(format!("/proc/self/fd/{}", self.file.as_raw_fd()))
                .map_err(|e| e.to_string())?
                .map(|e| {
                    e.map_err(|e| e.to_string()).and_then(|e| {
                        e.file_name()
                            .into_string()
                            .map_err(|_| "Non-UTF8 entry".into())
                    })
                })
                .collect()
        }
        pub fn remove_file(&self, s: &str) -> Result<(), String> {
            let c = name(s)?;
            let _checked = self.file(s, false)?;
            if unsafe { libc::unlinkat(self.file.as_raw_fd(), c.as_ptr(), 0) } < 0 {
                return Err(error());
            }
            Ok(())
        }
        pub fn remove_dir(&self, s: &str) -> Result<(), String> {
            let c = name(s)?;
            let _checked = self.child(s, false)?;
            if unsafe { libc::unlinkat(self.file.as_raw_fd(), c.as_ptr(), libc::AT_REMOVEDIR) } < 0
            {
                return Err(error());
            }
            Ok(())
        }
        pub fn inventory(&self, prefix: &str) -> Result<Vec<String>, String> {
            let mut files = Vec::new();
            for entry in self.entries()? {
                let full = format!("{prefix}{entry}");
                if let Ok(child) = self.child(&entry, false) {
                    files.push(format!("{full}/"));
                    files.extend(child.inventory(&format!("{full}/"))?)
                } else {
                    let _checked = self.file(&entry, false)?;
                    files.push(full)
                }
            }
            Ok(files)
        }
        pub fn purge_tree(&self) -> Result<(), String> {
            self.purge_tree_with(false)
        }
        pub fn purge_tree_with(&self, shred: bool) -> Result<(), String> {
            self.inventory("")?; // Refuse symlinks and hardlinks before any mutation.
            let mut entries = self.entries()?;
            // Preserve the recovery manifest and lock until payload cleanup succeeds.
            entries.sort_by_key(|s| {
                if s == ".lock" {
                    2
                } else if s == "manifest.json" {
                    1
                } else {
                    0
                }
            });
            for entry in entries {
                if let Ok(child) = self.child(&entry, false) {
                    child.purge_tree_with(shred)?;
                    self.remove_dir(&entry)?
                } else {
                    if shred && entry != ".lock" {
                        let file = self.file(&entry, false)?;
                        overwrite_file(file)?;
                    }
                    self.remove_file(&entry)?
                }
            }
            Ok(())
        }
    }
    fn overwrite_file(file: File) -> Result<(), String> {
        use std::{
            process::{Command, Stdio},
            time::{Duration, Instant},
        };
        // Inherit the checked inode as fd 0. No GUI-supplied pathname reaches shred.
        let mut child = Command::new(crate::process::executable("shred")?)
            .args([
                "--iterations=1",
                "--zero",
                "--exact",
                "--",
                "/proc/self/fd/0",
            ])
            .stdin(Stdio::from(file))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LC_ALL", "C")
            .spawn()
            .map_err(|e| format!("Cannot start shred: {e}"))?;
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                return if status.success() {
                    Ok(())
                } else {
                    Err(format!("shred failed: {status}; cleanup incomplete"))
                };
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err("shred timed out; cleanup incomplete".into());
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}
#[cfg(target_os = "linux")]
pub use linux::Dir;
