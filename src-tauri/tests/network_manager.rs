#![cfg(target_os = "linux")]
use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering},
        Arc,
    },
};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};
const DEVICE: &str = "/org/freedesktop/NetworkManager/Devices/1";
const AP: &str = "/org/freedesktop/NetworkManager/AccessPoint/1";
const PROFILE: &str = "/org/freedesktop/NetworkManager/Settings/1";
const ACTIVE: &str = "/org/freedesktop/NetworkManager/ActiveConnection/1";
const UUID: &str = "e36b601e-4f25-41d6-81ad-e4ed226741b2";
#[derive(Default)]
struct Link {
    state: AtomicU32,
    managed: AtomicBool,
    autoconnect: AtomicBool,
}
struct Manager {
    link: Arc<Link>,
}
#[zbus::interface(name = "org.freedesktop.NetworkManager")]
impl Manager {
    fn get_device_by_ip_iface(&self, interface: &str) -> zbus::fdo::Result<OwnedObjectPath> {
        if interface != "fixture0" {
            return Err(zbus::fdo::Error::UnknownObject("No such fixture".into()));
        }
        Ok(OwnedObjectPath::try_from(DEVICE).unwrap())
    }
    fn activate_connection(
        &self,
        profile: OwnedObjectPath,
        device: OwnedObjectPath,
        _specific: OwnedObjectPath,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        if profile.as_str() != PROFILE || device.as_str() != DEVICE {
            return Err(zbus::fdo::Error::InvalidArgs(
                "Unexpected activation target".into(),
            ));
        }
        self.link.state.store(100, Ordering::SeqCst);
        Ok(OwnedObjectPath::try_from(ACTIVE).unwrap())
    }
}
struct Device {
    link: Arc<Link>,
}
#[zbus::interface(name = "org.freedesktop.NetworkManager.Device")]
impl Device {
    #[zbus(property)]
    fn device_type(&self) -> u32 {
        2
    }
    #[zbus(property)]
    fn driver(&self) -> String {
        "fixture-driver".into()
    }
    #[zbus(property)]
    fn state(&self) -> u32 {
        self.link.state.load(Ordering::SeqCst)
    }
    #[zbus(property)]
    fn managed(&self) -> bool {
        self.link.managed.load(Ordering::SeqCst)
    }
    #[zbus(property)]
    fn set_managed(&mut self, value: bool) {
        self.link.managed.store(value, Ordering::SeqCst);
    }
    #[zbus(property)]
    fn autoconnect(&self) -> bool {
        self.link.autoconnect.load(Ordering::SeqCst)
    }
    #[zbus(property)]
    fn set_autoconnect(&mut self, value: bool) {
        self.link.autoconnect.store(value, Ordering::SeqCst);
    }
    #[zbus(property)]
    fn active_connection(&self) -> OwnedObjectPath {
        OwnedObjectPath::try_from(if self.state() == 100 { ACTIVE } else { "/" }).unwrap()
    }
    fn disconnect(&self) {
        self.link.state.store(30, Ordering::SeqCst);
        self.link.autoconnect.store(false, Ordering::SeqCst);
    }
}
struct Settings;
#[zbus::interface(name = "org.freedesktop.NetworkManager.Settings")]
impl Settings {
    fn get_connection_by_uuid(&self, uuid: &str) -> zbus::fdo::Result<OwnedObjectPath> {
        if uuid != UUID {
            return Err(zbus::fdo::Error::UnknownObject("Unknown profile".into()));
        }
        Ok(OwnedObjectPath::try_from(PROFILE).unwrap())
    }
}
struct Active;
#[zbus::interface(name = "org.freedesktop.NetworkManager.Connection.Active")]
impl Active {
    #[zbus(property)]
    fn uuid(&self) -> String {
        UUID.into()
    }
}
struct Wireless {
    last: Arc<AtomicI64>,
}
#[zbus::interface(name = "org.freedesktop.NetworkManager.Device.Wireless")]
impl Wireless {
    fn request_scan(&self, _options: HashMap<String, OwnedValue>) {
        self.last.store(100, Ordering::SeqCst);
    }
    fn get_all_access_points(&self) -> Vec<OwnedObjectPath> {
        vec![OwnedObjectPath::try_from(AP).unwrap()]
    }
    #[zbus(property)]
    fn last_scan(&self) -> i64 {
        self.last.load(Ordering::SeqCst)
    }
}
struct AccessPoint;
#[zbus::interface(name = "org.freedesktop.NetworkManager.AccessPoint")]
impl AccessPoint {
    #[zbus(property)]
    fn ssid(&self) -> Vec<u8> {
        b"OWNED\x00_LAB".to_vec()
    }
    #[zbus(property)]
    fn hw_address(&self) -> String {
        "02:DA:00:00:01:01".into()
    }
    #[zbus(property)]
    fn frequency(&self) -> u32 {
        2437
    }
    #[zbus(property)]
    fn strength(&self) -> u8 {
        73
    }
    #[zbus(property)]
    fn flags(&self) -> u32 {
        1
    }
    #[zbus(property)]
    fn wpa_flags(&self) -> u32 {
        0
    }
    #[zbus(property)]
    fn rsn_flags(&self) -> u32 {
        0x108
    }
}
#[tokio::test]
async fn actual_dbus_roundtrip_uses_advertised_capabilities() {
    let mut daemon = Command::new("dbus-daemon")
        .args(["--session", "--nofork", "--print-address=1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("dbus-daemon test dependency");
    let mut address = String::new();
    BufReader::new(daemon.stdout.take().unwrap())
        .read_line(&mut address)
        .unwrap();
    let address = address.trim().to_string();
    struct Guard {
        child: std::process::Child,
        previous: Option<std::ffi::OsString>,
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            match &self.previous {
                Some(value) => std::env::set_var("DBUS_SYSTEM_BUS_ADDRESS", value),
                None => std::env::remove_var("DBUS_SYSTEM_BUS_ADDRESS"),
            }
        }
    }
    let _guard = Guard {
        child: daemon,
        previous: std::env::var_os("DBUS_SYSTEM_BUS_ADDRESS"),
    };
    std::env::set_var("DBUS_SYSTEM_BUS_ADDRESS", &address);
    let link = Arc::new(Link {
        state: AtomicU32::new(100),
        managed: AtomicBool::new(true),
        autoconnect: AtomicBool::new(true),
    });
    let _server = zbus::connection::Builder::address(address.as_str())
        .unwrap()
        .name("org.freedesktop.NetworkManager")
        .unwrap()
        .serve_at(
            "/org/freedesktop/NetworkManager",
            Manager { link: link.clone() },
        )
        .unwrap()
        .serve_at(DEVICE, Device { link: link.clone() })
        .unwrap()
        .serve_at(
            DEVICE,
            Wireless {
                last: Arc::new(AtomicI64::new(-1)),
            },
        )
        .unwrap()
        .serve_at(AP, AccessPoint)
        .unwrap()
        .serve_at("/org/freedesktop/NetworkManager/Settings", Settings)
        .unwrap()
        .serve_at(ACTIVE, Active)
        .unwrap()
        .build()
        .await
        .unwrap();
    let result = dead_air::network::discover("fixture0", Arc::new(AtomicU64::new(0)))
        .await
        .unwrap();
    assert_eq!(result.networks.len(), 1);
    let network = &result.networks[0];
    assert_eq!(network.ssid, "OWNED_LAB");
    assert_eq!(network.quality, 73);
    assert_eq!(network.channel, 6);
    assert_eq!(network.security, "WPA2-Personal");
    assert_eq!(network.pmf, "Unknown");
    assert_eq!(result.source, "NetworkManager D-Bus");
    assert!(
        dead_air::network::discover("missing0", Arc::new(AtomicU64::new(0)))
            .await
            .is_err()
    );
    let before = dead_air::network::connection_snapshot("fixture0")
        .await
        .unwrap();
    assert_eq!(before.uuid.as_deref(), Some(UUID));
    assert!(before.autoconnect);
    dead_air::network::disconnect_for_capture("fixture0")
        .await
        .unwrap();
    assert_eq!(link.state.load(Ordering::SeqCst), 30);
    dead_air::network::adapter_control("fixture0", Some(false), true)
        .await
        .unwrap();
    dead_air::network::adapter_control("fixture0", Some(true), false)
        .await
        .unwrap();
    dead_air::network::restore_connection("fixture0", before.autoconnect, before.uuid.as_deref())
        .await
        .unwrap();
    let restored = dead_air::network::connection_snapshot("fixture0")
        .await
        .unwrap();
    assert_eq!(restored.state, 100);
    assert_eq!(restored.uuid, before.uuid);
    assert!(restored.autoconnect);
    link.state.store(50, Ordering::SeqCst);
    assert!(dead_air::network::disconnect_for_capture("fixture0")
        .await
        .is_err());
    assert_eq!(link.state.load(Ordering::SeqCst), 50);
}
