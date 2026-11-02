use crate::model::*;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

#[cfg(target_os = "linux")]
pub async fn discover(adapter: &str, generation: Arc<AtomicU64>) -> Result<Snapshot, String> {
    use zbus::{
        zvariant::{OwnedObjectPath, Value},
        Connection, Proxy,
    };
    if !valid_interface(adapter) {
        return Err("Select a valid discovery interface".into());
    }
    let token = generation.fetch_add(1, Ordering::SeqCst) + 1;
    let operation = async {
        let connection = Connection::system()
            .await
            .map_err(|e| format!("System D-Bus: {e}"))?;
        let manager = Proxy::new(
            &connection,
            "org.freedesktop.NetworkManager",
            "/org/freedesktop/NetworkManager",
            "org.freedesktop.NetworkManager",
        )
        .await
        .map_err(|e| e.to_string())?;
        let path: OwnedObjectPath = manager
            .call("GetDeviceByIpIface", &(adapter,))
            .await
            .map_err(|e| format!("NetworkManager interface {adapter}: {e}"))?;
        let device = Proxy::new(
            &connection,
            "org.freedesktop.NetworkManager",
            path.as_str(),
            "org.freedesktop.NetworkManager.Device",
        )
        .await
        .map_err(|e| e.to_string())?;
        let device_type: u32 = device
            .get_property("DeviceType")
            .await
            .map_err(|e| e.to_string())?;
        if device_type != 2 {
            return Err("Selected device is not Wi-Fi".into());
        }
        let driver: String = device.get_property("Driver").await.unwrap_or_default();
        let wifi = Proxy::new(
            &connection,
            "org.freedesktop.NetworkManager",
            path.as_str(),
            "org.freedesktop.NetworkManager.Device.Wireless",
        )
        .await
        .map_err(|e| e.to_string())?;
        let before: i64 = wifi
            .get_property("LastScan")
            .await
            .map_err(|e| e.to_string())?;
        let options: HashMap<&str, Value<'_>> = HashMap::new();
        let _: () = wifi.call("RequestScan", &(options,)).await.map_err(|e| {
            format!("Discovery request refused (permissions/radio/rate limit): {e}")
        })?;
        loop {
            if generation.load(Ordering::SeqCst) != token {
                return Err("Discovery cancelled".into());
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
            // A new proxy avoids relying on cached LastScan without a signal subscription.
            let fresh = Proxy::new(
                &connection,
                "org.freedesktop.NetworkManager",
                path.as_str(),
                "org.freedesktop.NetworkManager.Device.Wireless",
            )
            .await
            .map_err(|e| e.to_string())?;
            let last: i64 = fresh
                .get_property("LastScan")
                .await
                .map_err(|e| e.to_string())?;
            if last > before {
                break;
            }
        }
        let paths: Vec<OwnedObjectPath> = wifi
            .call("GetAllAccessPoints", &())
            .await
            .map_err(|e| e.to_string())?;
        let mut networks = Vec::new();
        for ap_path in paths.into_iter().take(512) {
            if generation.load(Ordering::SeqCst) != token {
                return Err("Discovery cancelled".into());
            }
            let ap = Proxy::new(
                &connection,
                "org.freedesktop.NetworkManager",
                ap_path.as_str(),
                "org.freedesktop.NetworkManager.AccessPoint",
            )
            .await
            .map_err(|e| e.to_string())?;
            let bytes: Vec<u8> = match ap.get_property("Ssid").await {
                Ok(v) => v,
                Err(_) => continue,
            };
            let bssid: String = match ap.get_property("HwAddress").await {
                Ok(v) => v,
                Err(_) => continue,
            };
            let frequency: u32 = ap
                .get_property("Frequency")
                .await
                .map_err(|e| e.to_string())?;
            let quality: u8 = ap
                .get_property("Strength")
                .await
                .map_err(|e| e.to_string())?;
            let flags: u32 = ap.get_property("Flags").await.map_err(|e| e.to_string())?;
            let wpa: u32 = ap
                .get_property("WpaFlags")
                .await
                .map_err(|e| e.to_string())?;
            let rsn: u32 = ap
                .get_property("RsnFlags")
                .await
                .map_err(|e| e.to_string())?;
            let cipher = match ((wpa | rsn) & 4 != 0, (wpa | rsn) & 8 != 0) {
                (true, true) => "TKIP / CCMP",
                (true, false) => "TKIP",
                (false, true) => "CCMP",
                _ => "Unknown",
            };
            networks.push(Network {
                ssid: String::from_utf8_lossy(&bytes)
                    .chars()
                    .filter(|c| {
                        !c.is_control()
                            && !('\u{202a}'..='\u{202e}').contains(c)
                            && !('\u{2066}'..='\u{2069}').contains(c)
                    })
                    .take(64)
                    .collect(),
                bssid,
                channel: channel(frequency),
                frequency,
                quality,
                security: security(flags, wpa, rsn),
                cipher: cipher.into(),
                pmf: "Unknown".into(),
            });
        }
        networks.sort_by(|a, b| b.quality.cmp(&a.quality));
        Ok(Snapshot {
            networks,
            adapter: adapter.into(),
            driver,
            source: "NetworkManager D-Bus".into(),
        })
    };
    tokio::time::timeout(Duration::from_secs(25), operation)
        .await
        .map_err(|_| "Discovery timed out; no fabricated results returned".to_string())?
}
#[cfg(not(target_os = "linux"))]
pub async fn discover(_: &str, _: Arc<AtomicU64>) -> Result<Snapshot, String> {
    Err("Live discovery requires Linux and NetworkManager".into())
}

pub async fn adapters() -> Result<Vec<Adapter>, String> {
    let text = crate::process::output("iw", &["dev"], 5).await?;
    let mut output = Vec::new();
    let mut phy = String::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(number) = line.strip_prefix("phy#") {
            phy = format!("phy{number}");
        }
        if let Some(name) = line.strip_prefix("Interface ") {
            let info = crate::process::output("iw", &["dev", name, "info"], 5).await?;
            let capabilities = crate::process::output("iw", &["phy", &phy, "info"], 5).await?;
            let mode = info
                .lines()
                .find_map(|l| l.trim().strip_prefix("type "))
                .unwrap_or("unknown")
                .to_string();
            let frequency = info
                .lines()
                .find(|l| l.trim().starts_with("channel "))
                .and_then(|l| l.split('(').nth(1))
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.parse().ok());
            let link = crate::process::output("iw", &["dev", name, "link"], 5)
                .await
                .unwrap_or_default();
            let driver = std::fs::read_link(format!("/sys/class/net/{name}/device/driver"))
                .ok()
                .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
                .unwrap_or_else(|| "Unknown".into());
            let frequencies: Vec<u32> = capabilities
                .lines()
                .filter_map(|line| {
                    let mut words = line.split_whitespace();
                    if words.next()? != "*" {
                        return None;
                    }
                    let frequency = words.next()?.parse().ok()?;
                    if words.next()? != "MHz" {
                        return None;
                    }
                    Some(frequency)
                })
                .collect();
            let mut bands = Vec::new();
            for (label, low, high) in [
                ("2.4 GHz", 2400, 2500),
                ("5 GHz", 4900, 5924),
                ("6 GHz", 5925, 7125),
            ] {
                if frequencies.iter().any(|f| (*f >= low) && (*f <= high)) {
                    bands.push(label.into());
                }
            }
            output.push(Adapter {
                name: name.into(),
                driver,
                phy: phy.clone(),
                monitor_supported: mode == "monitor"
                    || capabilities.lines().any(|l| l.trim() == "* monitor"),
                mode,
                frequency,
                connected: link.contains("Connected to"),
                bands,
            });
        }
    }
    Ok(output)
}

#[cfg(target_os = "linux")]
pub async fn adapter_control(
    name: &str,
    managed: Option<bool>,
    require_idle: bool,
) -> Result<bool, String> {
    use zbus::{zvariant::OwnedObjectPath, Connection, Proxy};
    if !valid_interface(name) {
        return Err("Invalid interface".into());
    }
    let operation = async {
        let bus = Connection::system().await.map_err(|e| e.to_string())?;
        let manager = Proxy::new(
            &bus,
            "org.freedesktop.NetworkManager",
            "/org/freedesktop/NetworkManager",
            "org.freedesktop.NetworkManager",
        )
        .await
        .map_err(|e| e.to_string())?;
        let path: OwnedObjectPath = manager
            .call("GetDeviceByIpIface", &(name,))
            .await
            .map_err(|e| format!("NetworkManager device lookup: {e}"))?;
        let device = Proxy::new(
            &bus,
            "org.freedesktop.NetworkManager",
            path.as_str(),
            "org.freedesktop.NetworkManager.Device",
        )
        .await
        .map_err(|e| e.to_string())?;
        let state: u32 = device
            .get_property("State")
            .await
            .map_err(|e| e.to_string())?;
        if require_idle && state >= 40 && state <= 110 {
            return Err("Selected adapter is connected or changing connection state. Choose an idle secondary radio.".into());
        }
        let original: bool = device
            .get_property("Managed")
            .await
            .map_err(|e| e.to_string())?;
        if let Some(value) = managed {
            device
                .set_property("Managed", value)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(original)
    };
    tokio::time::timeout(Duration::from_secs(5), operation)
        .await
        .map_err(|_| "NetworkManager adapter control timeout".to_string())?
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ConnectionSnapshot {
    pub managed: bool,
    pub autoconnect: bool,
    pub state: u32,
    pub uuid: Option<String>,
}
#[cfg(target_os = "linux")]
async fn device_proxy<'a>(
    bus: &'a zbus::Connection,
    name: &str,
) -> Result<zbus::Proxy<'a>, String> {
    use zbus::zvariant::OwnedObjectPath;
    if !valid_interface(name) {
        return Err("Invalid interface".into());
    }
    let manager = zbus::Proxy::new(
        bus,
        "org.freedesktop.NetworkManager",
        "/org/freedesktop/NetworkManager",
        "org.freedesktop.NetworkManager",
    )
    .await
    .map_err(|e| e.to_string())?;
    let path: OwnedObjectPath = manager
        .call("GetDeviceByIpIface", &(name,))
        .await
        .map_err(|e| e.to_string())?;
    zbus::Proxy::new_owned(
        bus.clone(),
        "org.freedesktop.NetworkManager",
        path,
        "org.freedesktop.NetworkManager.Device",
    )
    .await
    .map_err(|e| e.to_string())
}
#[cfg(target_os = "linux")]
pub async fn connection_snapshot(name: &str) -> Result<ConnectionSnapshot, String> {
    let operation = async {
        let bus = zbus::Connection::system()
            .await
            .map_err(|e| e.to_string())?;
        let device = device_proxy(&bus, name).await?;
        let kind: u32 = device
            .get_property("DeviceType")
            .await
            .map_err(|e| e.to_string())?;
        if kind != 2 {
            return Err("Selected device is not Wi-Fi".into());
        }
        let state: u32 = device
            .get_property("State")
            .await
            .map_err(|e| e.to_string())?;
        let managed = device
            .get_property("Managed")
            .await
            .map_err(|e| e.to_string())?;
        let autoconnect = device
            .get_property("Autoconnect")
            .await
            .map_err(|e| e.to_string())?;
        let active: zbus::zvariant::OwnedObjectPath = device
            .get_property("ActiveConnection")
            .await
            .map_err(|e| e.to_string())?;
        let uuid = if active.as_str() != "/" {
            let proxy = zbus::Proxy::new(
                &bus,
                "org.freedesktop.NetworkManager",
                active,
                "org.freedesktop.NetworkManager.Connection.Active",
            )
            .await
            .map_err(|e| e.to_string())?;
            Some(
                proxy
                    .get_property::<String>("Uuid")
                    .await
                    .map_err(|e| e.to_string())?,
            )
        } else {
            None
        };
        Ok(ConnectionSnapshot {
            managed,
            autoconnect,
            state,
            uuid,
        })
    };
    tokio::time::timeout(Duration::from_secs(5), operation)
        .await
        .map_err(|_| "NetworkManager snapshot timed out")?
}
#[cfg(target_os = "linux")]
pub async fn disconnect_for_capture(name: &str) -> Result<(), String> {
    let before = connection_snapshot(name).await?;
    if before.state != 100 && before.state >= 40 && before.state <= 110 {
        return Err("Adapter is changing state; wait until connected or idle".into());
    }
    if before.state != 100 {
        return Ok(());
    }
    let bus = zbus::Connection::system()
        .await
        .map_err(|e| e.to_string())?;
    let device = device_proxy(&bus, name).await?;
    tokio::time::timeout(
        Duration::from_secs(5),
        device.call::<_, _, ()>("Disconnect", &()),
    )
    .await
    .map_err(|_| "Wi-Fi disconnection request timed out")?
    .map_err(|e| e.to_string())?;
    for _ in 0..100 {
        let state: u32 = device
            .get_property("State")
            .await
            .map_err(|e| e.to_string())?;
        if state < 40 || state == 120 {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err("Selected Wi-Fi did not disconnect; capture not started".into())
}
#[cfg(target_os = "linux")]
pub async fn restore_connection(
    name: &str,
    autoconnect: bool,
    uuid: Option<&str>,
) -> Result<(), String> {
    let bus = zbus::Connection::system()
        .await
        .map_err(|e| e.to_string())?;
    let device = device_proxy(&bus, name).await?;
    device
        .set_property("Autoconnect", autoconnect)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(uuid) = uuid {
        if uuid::Uuid::parse_str(uuid).is_err() {
            return Err("Invalid saved connection UUID".into());
        }
        let before = connection_snapshot(name).await?;
        if before.uuid.as_deref() == Some(uuid) && before.state == 100 {
            return Ok(());
        }
        if before.uuid.is_some() && before.uuid.as_deref() != Some(uuid) {
            return Err(
                "Another connection now owns this adapter; automatic reconnection refused".into(),
            );
        }
        let settings = zbus::Proxy::new(
            &bus,
            "org.freedesktop.NetworkManager",
            "/org/freedesktop/NetworkManager/Settings",
            "org.freedesktop.NetworkManager.Settings",
        )
        .await
        .map_err(|e| e.to_string())?;
        let profile: zbus::zvariant::OwnedObjectPath = settings
            .call("GetConnectionByUuid", &(uuid,))
            .await
            .map_err(|_| "Saved Wi-Fi profile is no longer available")?;
        let manager = zbus::Proxy::new(
            &bus,
            "org.freedesktop.NetworkManager",
            "/org/freedesktop/NetworkManager",
            "org.freedesktop.NetworkManager",
        )
        .await
        .map_err(|e| e.to_string())?;
        let any = zbus::zvariant::OwnedObjectPath::try_from("/").unwrap();
        let _: zbus::zvariant::OwnedObjectPath = tokio::time::timeout(
            Duration::from_secs(5),
            manager.call("ActivateConnection", &(profile, device.path(), any)),
        )
        .await
        .map_err(|_| "Wi-Fi activation request timed out")?
        .map_err(|_| "NetworkManager could not reactivate the saved Wi-Fi profile")?;
        for _ in 0..100 {
            let after = connection_snapshot(name).await?;
            if after.state == 100 && after.uuid.as_deref() == Some(uuid) {
                device
                    .set_property("Autoconnect", autoconnect)
                    .await
                    .map_err(|e| e.to_string())?;
                return Ok(());
            }
            if after.state == 120 {
                return Err(
                    "Saved Wi-Fi activation failed; check AP availability and credentials".into(),
                );
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        return Err("Wi-Fi reconnection not confirmed by NetworkManager; retry recovery".into());
    }
    Ok(())
}
