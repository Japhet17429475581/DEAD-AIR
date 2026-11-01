//! MAC-only profiles. OUI selection does not emulate an OS or device model.
use serde::Serialize;
use std::time::Duration;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    process::{Child, ChildStdin},
};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MacStatus {
    pub active: bool,
    pub adapter: String,
    pub profile: String,
    pub original: String,
    pub assigned: String,
    pub current: String,
    pub error: Option<String>,
}
impl Default for MacStatus {
    fn default() -> Self {
        Self {
            active: false,
            adapter: String::new(),
            profile: String::new(),
            original: String::new(),
            assigned: String::new(),
            current: String::new(),
            error: None,
        }
    }
}
pub fn profile_mac(profile: &str) -> Result<String, String> {
    let random = uuid::Uuid::new_v4();
    let bytes = random.as_bytes();
    let prefix = match profile {
        "private" => [bytes[0] & 0xfc | 2, bytes[1], bytes[2]],
        "apple" => [0xf0, 0xd1, 0xa9],
        "samsung" => [0x28, 0x83, 0x35],
        "intel" => [0x3c, 0x6a, 0xa7],
        "raspberry" => [0xb8, 0x27, 0xeb],
        _ => return Err("Unknown MAC profile".into()),
    };
    Ok([
        prefix[0], prefix[1], prefix[2], bytes[13], bytes[14], bytes[15],
    ]
    .iter()
    .map(|b| format!("{b:02X}"))
    .collect::<Vec<_>>()
    .join(":"))
}
pub fn current_mac(adapter: &str) -> Result<String, String> {
    if !crate::model::valid_interface(adapter) {
        return Err("Invalid interface".into());
    }
    let address = std::fs::read_to_string(format!("/sys/class/net/{adapter}/address"))
        .map_err(|e| e.to_string())?;
    crate::model::parse_mac(address.trim())?;
    Ok(address.trim().to_uppercase())
}
pub struct MacService {
    child: Option<Child>,
    input: Option<ChildStdin>,
    reader: Option<BufReader<tokio::process::ChildStdout>>,
    state: MacStatus,
}
impl Default for MacService {
    fn default() -> Self {
        Self {
            child: None,
            input: None,
            reader: None,
            state: MacStatus::default(),
        }
    }
}
async fn message(
    reader: &mut BufReader<tokio::process::ChildStdout>,
    seconds: u64,
) -> Result<serde_json::Value, String> {
    let mut bytes = Vec::new();
    tokio::time::timeout(
        Duration::from_secs(seconds),
        reader.take(8192).read_until(b'\n', &mut bytes),
    )
    .await
    .map_err(|_| "MAC helper timed out; inspect restoration")?
    .map_err(|e| e.to_string())?;
    if bytes.is_empty() || bytes.len() >= 8192 {
        return Err("MAC helper closed or exceeded protocol bounds".into());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if value["kind"] == "error" {
        return Err(value["message"]
            .as_str()
            .unwrap_or("MAC helper failed")
            .into());
    }
    Ok(value)
}
impl MacService {
    pub fn status(&self) -> MacStatus {
        let mut state = self.state.clone();
        if state.active {
            match current_mac(&state.adapter) {
                Ok(mac) => state.current = mac,
                Err(e) => state.error = Some(e),
            }
        }
        state
    }
    pub async fn start(&mut self, adapter: &str, profile: &str) -> Result<MacStatus, String> {
        if self.child.is_some() || self.state.error.is_some() {
            return Err("Restore the previous MAC operation first".into());
        }
        if !crate::model::valid_interface(adapter) {
            return Err("Invalid interface".into());
        }
        profile_mac(profile)?;
        let token = uuid::Uuid::new_v4().to_string();
        let mut child = crate::privilege::command(&["mac-profile", adapter, profile, &token])
            .await?
            .stdin(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        self.input = child.stdin.take();
        self.reader = Some(BufReader::new(
            child.stdout.take().ok_or("MAC helper stdout missing")?,
        ));
        self.child = Some(child);
        match message(self.reader.as_mut().unwrap(), 120).await {
            Ok(value) if value["kind"] == "mac-ready" => {
                self.state = MacStatus {
                    active: true,
                    adapter: adapter.into(),
                    profile: profile.into(),
                    original: value["original"].as_str().unwrap_or("").into(),
                    assigned: value["assigned"].as_str().unwrap_or("").into(),
                    current: current_mac(adapter)?,
                    error: None,
                };
                Ok(self.status())
            }
            result => {
                let error = result.err().unwrap_or("Unexpected MAC helper reply".into());
                self.input.take();
                let cleanup = self.stop().await;
                Err(match cleanup {
                    Ok(_) => error,
                    Err(e) => format!("{error}; restoration: {e}"),
                })
            }
        }
    }
    pub async fn stop(&mut self) -> Result<MacStatus, String> {
        self.input.take();
        if let Some(mut child) = self.child.take() {
            let reply = if let Some(reader) = self.reader.as_mut() {
                message(reader, 30).await
            } else {
                Err("Missing MAC restoration reply".into())
            };
            let exit = tokio::time::timeout(Duration::from_secs(3), child.wait()).await;
            self.reader.take();
            self.state.active = false;
            if !matches!(reply,Ok(ref v) if v["kind"]=="mac-restored")
                || !matches!(exit,Ok(Ok(ref code)) if code.success())
            {
                self.state.error =
                    Some("MAC restoration not confirmed; use RESTORE STALE MAC in Adapters".into());
                return Err(self.state.error.clone().unwrap());
            }
            self.state = MacStatus::default();
        }
        if let Some(error) = &self.state.error {
            return Err(error.clone());
        }
        Ok(self.status())
    }
    pub fn recovered(&mut self) {
        if self.child.is_none() {
            self.state = MacStatus::default();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_profiles_are_valid_unicast_and_private_sets_local_bit() {
        for profile in ["private", "apple", "samsung", "intel", "raspberry"] {
            for _ in 0..32 {
                let mac = crate::model::parse_mac(&profile_mac(profile).unwrap()).unwrap();
                assert_eq!(mac[0] & 1, 0);
                if profile == "private" {
                    assert_eq!(mac[0] & 2, 2)
                }
            }
        }
        assert!(profile_mac("unknown").is_err());
        assert!(current_mac("../eth0").is_err());
    }
}
