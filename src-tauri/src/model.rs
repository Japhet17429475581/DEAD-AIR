use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Network {
    pub ssid: String,
    pub bssid: String,
    pub channel: u32,
    pub frequency: u32,
    pub quality: u8,
    pub security: String,
    pub cipher: String,
    pub pmf: String,
}
#[derive(Clone, Default, Serialize)]
pub struct Snapshot {
    pub networks: Vec<Network>,
    pub adapter: String,
    pub driver: String,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Adapter {
    pub name: String,
    pub driver: String,
    pub phy: String,
    pub mode: String,
    pub frequency: Option<u32>,
    pub monitor_supported: bool,
    pub connected: bool,
    pub bands: Vec<String>,
}
#[derive(Clone, Serialize)]
pub struct ToolStatus {
    pub name: String,
    pub available: bool,
    pub version: String,
    pub detail: String,
}
#[derive(Clone, Serialize)]
pub struct Diagnostics {
    pub platform: String,
    pub tools: Vec<ToolStatus>,
    pub adapters: Vec<Adapter>,
    pub errors: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Policy {
    MemoryOnly,
    RamOnly,
    Temporary,
    Persistent,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub policy: Policy,
    pub state: String,
    pub path: Option<String>,
    pub files: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Packet {
    pub time: f64,
    pub source: String,
    pub destination: String,
    pub station: String,
    pub kind: String,
    pub message: Option<u8>,
    pub replay: Option<String>,
    pub signal: Option<i8>,
    pub bytes: usize,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Exchange {
    pub station: String,
    pub messages: Vec<u8>,
    pub complete: bool,
}
#[derive(Clone, Serialize)]
pub struct CaptureStatus {
    pub running: bool,
    pub status: String,
    pub packets: Vec<Packet>,
    pub count: u64,
    pub dropped: u64,
    pub error: Option<String>,
    pub exchanges: Vec<Exchange>,
    pub pmkids: Vec<crate::pmkid::Observation>,
    pub progress: Option<crate::capture_health::CaptureProgress>,
    #[serde(rename = "stopReason")]
    pub stop_reason: Option<String>,
}
impl Default for CaptureStatus {
    fn default() -> Self {
        Self {
            running: false,
            status: "IDLE".into(),
            packets: vec![],
            count: 0,
            dropped: 0,
            error: None,
            exchanges: vec![],
            pmkids: vec![],
            progress: None,
            stop_reason: None,
        }
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LabStatus {
    pub running: bool,
    pub status: String,
    pub lines: Vec<String>,
    pub exit_code: Option<i32>,
}
impl Default for LabStatus {
    fn default() -> Self {
        Self {
            running: false,
            status: "IDLE".into(),
            lines: vec![],
            exit_code: None,
        }
    }
}

pub fn valid_interface(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 15
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
pub fn parse_mac(value: &str) -> Result<[u8; 6], String> {
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() != 6 || parts.iter().any(|p| p.len() != 2) {
        return Err("Invalid exact BSSID".into());
    }
    let mut mac = [0; 6];
    for (i, part) in parts.iter().enumerate() {
        mac[i] = u8::from_str_radix(part, 16).map_err(|_| "Invalid BSSID")?;
    }
    if mac[0] & 1 != 0 || mac == [0; 6] {
        return Err("BSSID must be a unicast radio address".into());
    }
    Ok(mac)
}
pub fn channel(f: u32) -> u32 {
    match f {
        2484 => 14,
        2412..=2472 => (f - 2407) / 5,
        5000..=5895 => (f - 5000) / 5,
        5955..=7115 => (f - 5950) / 5,
        _ => 0,
    }
}
pub fn security(flags: u32, wpa: u32, rsn: u32) -> String {
    if rsn & 0x2000 != 0 {
        "WPA3-Enterprise (Suite-B-192)"
    } else if rsn & 0x400 != 0 && rsn & 0x100 != 0 {
        "WPA2/WPA3"
    } else if rsn & 0x400 != 0 {
        "WPA3-Personal"
    } else if rsn & 0x1000 != 0 {
        "OWE transition"
    } else if rsn & 0x800 != 0 {
        "OWE"
    } else if rsn & 0x200 != 0 {
        "WPA2-Enterprise / EAP"
    } else if rsn & 0x100 != 0 {
        "WPA2-Personal"
    } else if wpa != 0 {
        "WPA legacy"
    } else if flags & 1 != 0 {
        "Protected / unknown"
    } else {
        "Open"
    }
    .into()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_untrusted_identifiers() {
        assert!(!valid_interface("wlan0;id"));
        assert!(!valid_interface("../wlan0"));
        assert!(valid_interface("wlan1"));
        assert!(parse_mac("ff:ff:ff:ff:ff:ff").is_err());
        assert!(parse_mac("02:00:00:00:00:01").is_ok());
    }
    #[test]
    fn frequencies_are_not_rssi() {
        assert_eq!(channel(2437), 6);
        assert_eq!(channel(5180), 36);
        assert_eq!(channel(5955), 1);
        assert_eq!(channel(0), 0);
    }
    #[test]
    fn advertised_akm_flags() {
        assert_eq!(security(1, 0, 0x100), "WPA2-Personal");
        assert_eq!(security(1, 0, 0x400), "WPA3-Personal");
        assert_eq!(security(1, 0, 0x500), "WPA2/WPA3");
        assert_eq!(security(0, 0, 0x800), "OWE");
        assert_eq!(security(0, 0, 0), "Open");
        assert!(security(1, 0, 0x2000).starts_with("WPA3-Enterprise"));
    }
}
