//! Capture planning and exact channel restoration for an existing monitor interface.
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq)]
pub enum CaptureAdapterMode {
    Managed,
    ExistingMonitor,
}

pub fn interface_mode(info: &str) -> Result<CaptureAdapterMode, String> {
    match info.lines().find_map(|line| line.trim().strip_prefix("type ")) {
        Some("managed") => Ok(CaptureAdapterMode::Managed),
        Some("monitor") => Ok(CaptureAdapterMode::ExistingMonitor),
        Some(mode) => Err(format!("Capture requires managed or monitor mode, not {mode}. AP/mesh interfaces are not changed.")),
        None => Err("iw did not report the interface type; refresh the adapter inventory".into()),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MonitorChannel {
    pub frequency: u32,
    pub width: String,
    pub center1: u32,
    pub center2: Option<u32>,
}

impl MonitorChannel {
    pub fn from_info(info: &str) -> Result<Option<Self>, String> {
        let Some(line) = info
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with("channel "))
        else {
            return Ok(None);
        };
        let number = |text: &str| -> Result<u32, String> {
            text.split_whitespace()
                .next()
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| "Cannot record the original monitor channel".into())
        };
        let frequency = number(line.split_once('(').ok_or("Missing monitor frequency")?.1)?;
        let width_text = line
            .split(',')
            .map(str::trim)
            .find_map(|s| s.strip_prefix("width: "))
            .ok_or("Cannot record the original monitor channel width")?;
        let width = if width_text.contains("no HT") {
            "NOHT"
        } else {
            width_text
                .split_whitespace()
                .next()
                .ok_or("Missing monitor width")?
        }
        .to_string();
        let field = |name: &str| -> Result<Option<u32>, String> {
            line.split(',')
                .map(str::trim)
                .find_map(|s| s.strip_prefix(name))
                .map(number)
                .transpose()
        };
        let channel = Self {
            frequency,
            width,
            center1: field("center1: ")?.unwrap_or(frequency),
            center2: field("center2: ")?,
        };
        channel.arguments()?;
        Ok(Some(channel))
    }

    /// No unvalidated journal string is forwarded as an iw argument.
    pub fn arguments(&self) -> Result<Vec<String>, String> {
        if !(2300..=7200).contains(&self.frequency)
            || !(2300..=7200).contains(&self.center1)
            || self.center2.is_some_and(|f| !(2300..=7200).contains(&f))
        {
            return Err("Invalid original monitor channel frequency".into());
        }
        if self.width == "NOHT" {
            return Ok(vec![self.frequency.to_string(), "NOHT".into()]);
        }
        if !matches!(
            self.width.as_str(),
            "5" | "10" | "20" | "40" | "80" | "80+80" | "160" | "320"
        ) || (self.width == "80+80") != self.center2.is_some()
        {
            return Err("Unsupported original monitor channel width; restore the card to managed mode first".into());
        }
        let mut args = vec![
            self.frequency.to_string(),
            self.width.clone(),
            self.center1.to_string(),
        ];
        if let Some(center2) = self.center2 {
            args.push(center2.to_string());
        }
        Ok(args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn managed_and_existing_monitor_are_allowed_but_other_roles_are_not() {
        assert_eq!(
            interface_mode("Interface wlan0\n type managed\n").unwrap(),
            CaptureAdapterMode::Managed
        );
        assert_eq!(
            interface_mode("Interface wlan0mon\n type monitor\n").unwrap(),
            CaptureAdapterMode::ExistingMonitor
        );
        for role in ["AP", "mesh point", "P2P-client"] {
            assert!(interface_mode(&format!("type {role}")).is_err());
        }
        assert!(interface_mode("").is_err());
    }
    #[test]
    fn existing_monitor_channel_preserves_width_and_both_centers() {
        let c = MonitorChannel::from_info("type monitor\n channel 36 (5180 MHz), width: 80+80 MHz, center1: 5210 MHz, center2: 5530 MHz").unwrap().unwrap();
        assert_eq!(c.arguments().unwrap(), ["5180", "80+80", "5210", "5530"]);
        let c = MonitorChannel::from_info(
            "channel 6 (2437 MHz), width: 20 MHz (no HT), center1: 2437 MHz",
        )
        .unwrap()
        .unwrap();
        assert_eq!(c.arguments().unwrap(), ["2437", "NOHT"]);
        let c =
            MonitorChannel::from_info("channel 36 (5180 MHz), width: 40 MHz, center1: 5190 MHz")
                .unwrap()
                .unwrap();
        assert_eq!(c.arguments().unwrap(), ["5180", "40", "5190"]);
    }
    #[test]
    fn unknown_channel_is_distinct_from_malformed_or_unrestorable_channel() {
        assert_eq!(MonitorChannel::from_info("type monitor\n").unwrap(), None);
        assert!(MonitorChannel::from_info("channel 6 (oops MHz), width: 20 MHz").is_err());
        assert!(MonitorChannel::from_info("channel 6 (2437 MHz), width: 999 MHz").is_err());
        assert!(MonitorChannel::from_info(
            "channel 36 (5180 MHz), width: 80+80 MHz, center1: 5210 MHz"
        )
        .is_err());
    }
    #[test]
    fn untrusted_journal_cannot_supply_extra_iw_arguments() {
        let c = MonitorChannel {
            frequency: 2437,
            width: "20; reboot".into(),
            center1: 2437,
            center2: None,
        };
        assert!(c.arguments().is_err());
    }
}
