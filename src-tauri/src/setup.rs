use serde::Serialize;
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupPlan {
    pub distro: String,
    pub manager: String,
    pub packages: Vec<String>,
    pub missing: Vec<String>,
    pub helper_available: bool,
    pub profile: String,
}
pub fn distro_plan(
    os_release: &str,
    profile: &str,
) -> Result<(String, String, Vec<String>), String> {
    if profile == "all" {
        let (id, manager, mut packages) = distro_plan(os_release, "wireless")?;
        packages.push("hashcat".into());
        return Ok((id, manager, packages));
    }
    if !matches!(profile, "wireless" | "hashcat" | "cpu") {
        return Err("Unknown dependency profile".into());
    }
    let id = os_release
        .lines()
        .find_map(|l| l.strip_prefix("ID="))
        .unwrap_or("")
        .trim_matches('"')
        .to_string();
    let (manager,packages)=match(id.as_str(),profile){
        ("ubuntu"|"debian","wireless")=>("apt-get",vec!["network-manager","iw","iproute2","aircrack-ng","ethtool","pciutils","usbutils","kmod","coreutils","xdg-utils"]),
        ("fedora","wireless")=>("dnf",vec!["NetworkManager-wifi","iw","iproute","aircrack-ng","ethtool","pciutils","usbutils","kmod","coreutils","xdg-utils"]),
        ("arch","wireless")=>("pacman",vec!["networkmanager","iw","iproute2","aircrack-ng","ethtool","pciutils","usbutils","kmod","coreutils","xdg-utils"]),
        ("ubuntu"|"debian","hashcat")=>("apt-get",vec!["hashcat"]),("fedora","hashcat")=>("dnf",vec!["hashcat"]),("arch","hashcat")=>("pacman",vec!["hashcat"]),
        ("ubuntu"|"debian","cpu")=>("apt-get",vec!["pocl-opencl-icd","clinfo"]),("fedora","cpu")=>("dnf",vec!["pocl","clinfo"]),("arch","cpu")=>("pacman",vec!["pocl","clinfo"]),
        _=>return Err(format!("Automatic installation is not supported on {id}; supported: Ubuntu, Debian, Fedora, Arch"))
    };
    Ok((
        id,
        manager.into(),
        packages.into_iter().map(String::from).collect(),
    ))
}
pub fn plan(profile: &str) -> Result<SetupPlan, String> {
    let os = std::fs::read_to_string("/etc/os-release")
        .map_err(|_| "Dependency installation requires Linux")?;
    let (distro, manager, packages) = distro_plan(&os, profile)?;
    let binaries = match profile {
        "wireless" => vec![
            "nmcli",
            "iw",
            "ip",
            "airmon-ng",
            "airodump-ng",
            "ethtool",
            "lspci",
            "lsusb",
            "lsmod",
            "shred",
            "xdg-open",
        ],
        "all" => vec![
            "nmcli",
            "iw",
            "ip",
            "airmon-ng",
            "airodump-ng",
            "ethtool",
            "lspci",
            "lsusb",
            "lsmod",
            "shred",
            "xdg-open",
            "hashcat",
        ],
        "cpu" => vec!["clinfo"],
        _ => vec!["hashcat"],
    };
    let mut missing: Vec<String> = binaries
        .into_iter()
        .filter(|name| crate::process::executable(name).is_err())
        .map(String::from)
        .collect();
    if profile == "cpu" && !std::path::Path::new("/etc/OpenCL/vendors/pocl.icd").exists() {
        missing.push("PoCL CPU runtime".into());
    }
    Ok(SetupPlan {
        distro,
        manager,
        packages,
        missing,
        helper_available: std::path::Path::new(crate::capture::HELPER).is_file(),
        profile: profile.into(),
    })
}
/// Only the installed polkit helper calls this. The GUI supplies a profile,
/// never package names, repositories, download URLs, or command fragments.
#[cfg(target_os = "linux")]
pub fn install(profile: &str) -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    if unsafe { libc::geteuid() } != 0 {
        return Err("Package installation requires polkit authorization".into());
    }
    let p = plan(profile)?;
    fn execute(binary: &str, args: &[String]) -> Result<(), String> {
        let mut command = Command::new(crate::process::executable(binary)?);
        command
            .args(args)
            .env_clear()
            .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
            .env("LC_ALL", "C")
            .env("DEBIAN_FRONTEND", "noninteractive")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        unsafe {
            command.pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            });
        }
        let mut child = command.spawn().map_err(|e| e.to_string())?;
        let deadline = Instant::now() + Duration::from_secs(1200);
        loop {
            if let Some(exit) = child.try_wait().map_err(|e| e.to_string())? {
                return if exit.success() {
                    Ok(())
                } else {
                    Err(format!("Package manager failed ({exit}). Check internet access, repository configuration and package-manager locks, then retry."))
                };
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(
                    "Package operation timed out. Inspect package-manager status before retrying."
                        .into(),
                );
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    if p.manager == "apt-get" {
        execute(
            "apt-get",
            &["-o".into(), "Acquire::Retries=3".into(), "update".into()],
        )?
    }
    let mut args: Vec<String> = match p.manager.as_str() {
        "apt-get" => vec!["install", "-y", "--no-install-recommends"],
        "dnf" => vec!["install", "-y"],
        "pacman" => vec!["-S", "--needed", "--noconfirm"],
        _ => return Err("Unsupported package manager".into()),
    }
    .into_iter()
    .map(String::from)
    .collect();
    args.extend(p.packages);
    execute(&p.manager, &args)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distro_packages_are_explicit() {
        assert_eq!(
            distro_plan("ID=debian", "wireless").unwrap().2[0],
            "network-manager"
        );
        assert_eq!(
            distro_plan("ID=fedora", "wireless").unwrap().2[0],
            "NetworkManager-wifi"
        );
        assert_eq!(distro_plan("ID=arch", "wireless").unwrap().1, "pacman");
        assert!(distro_plan("ID=unknown", "wireless").is_err());
        assert!(distro_plan("ID=ubuntu", "; rm -rf /").is_err());
    }
    #[test]
    fn lab_never_installs_drivers_or_wordlists() {
        for id in ["ubuntu", "debian", "fedora", "arch"] {
            assert_eq!(
                distro_plan(&format!("ID={id}"), "hashcat").unwrap().2,
                vec!["hashcat"]
            );
        }
    }
}
