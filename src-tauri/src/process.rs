use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command};

pub fn executable(name: &str) -> Result<PathBuf, String> {
    if !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return Err("Invalid executable name".into());
    }
    for directory in ["/usr/bin", "/usr/sbin", "/bin", "/sbin"] {
        let path = PathBuf::from(directory).join(name);
        if path.is_file() {
            return Ok(path);
        }
    }
    Err(format!(
        "{name} is not installed in a system executable directory. Run scripts/doctor.sh."
    ))
}
pub fn command(name: &str) -> Result<Command, String> {
    let mut command = Command::new(executable(name)?);
    command
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(target_os = "linux")]
    unsafe {
        command.pre_exec(|| {
            let parent = libc::getppid();
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if parent == 1 || libc::getppid() != parent {
                return Err(std::io::Error::other("Parent exited"));
            }
            Ok(())
        });
    }
    Ok(command)
}
pub async fn output(name: &str, args: &[&str], seconds: u64) -> Result<String, String> {
    let mut cmd = command(name)?;
    cmd.args(args);
    output_command(cmd, name, seconds).await
}
pub async fn output_command(mut cmd: Command, name: &str, seconds: u64) -> Result<String, String> {
    let mut child = cmd.spawn().map_err(|e| format!("{name}: {e}"))?;
    let stdout = child.stdout.take().ok_or("Missing stdout")?;
    let stderr = child.stderr.take().ok_or("Missing stderr")?;
    let work = async {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let mut limited_stdout = stdout.take(262145);
        let mut limited_stderr = stderr.take(32769);
        let (a, b, status) = tokio::join!(
            limited_stdout.read_to_end(&mut out),
            limited_stderr.read_to_end(&mut err),
            child.wait()
        );
        a.map_err(|e| e.to_string())?;
        b.map_err(|e| e.to_string())?;
        let status = status.map_err(|e| e.to_string())?;
        if out.len() > 262144 || err.len() > 32768 {
            return Err(format!("{name}: output limit exceeded"));
        }
        if !status.success() {
            return Err(format!(
                "{name} failed ({status}): {}",
                String::from_utf8_lossy(if err.is_empty() { &out } else { &err })
                    .chars()
                    .take(1500)
                    .collect::<String>()
            ));
        }
        Ok(String::from_utf8_lossy(&out).into_owned())
    };
    match tokio::time::timeout(Duration::from_secs(seconds), work).await {
        Ok(result) => result,
        Err(_) => {
            let _ = child.kill().await;
            Err(format!("{name}: operation timed out after {seconds}s"))
        }
    }
}
