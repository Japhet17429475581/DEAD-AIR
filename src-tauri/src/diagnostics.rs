use crate::model::*;
pub async fn check() -> Diagnostics {
    let mut result = Diagnostics {
        platform: std::env::consts::OS.into(),
        tools: vec![],
        adapters: vec![],
        errors: vec![],
    };
    for (name, binary, args) in [
        ("NetworkManager", "nmcli", vec!["--version"]),
        ("iw", "iw", vec!["--version"]),
        ("Hashcat", "hashcat", vec!["--version"]),
        ("polkit", "pkexec", vec!["--version"]),
    ] {
        match crate::process::output(binary, &args, 6).await {
            Ok(version) => result.tools.push(ToolStatus {
                name: name.into(),
                available: true,
                version: version.lines().next().unwrap_or("").into(),
                detail: "Installed system binary".into(),
            }),
            Err(error) => result.tools.push(ToolStatus {
                name: name.into(),
                available: false,
                version: "—".into(),
                detail: error,
            }),
        }
    }
    for name in ["airmon-ng", "airodump-ng"] {
        let available = crate::process::executable(name).is_ok();
        result.tools.push(ToolStatus {
            name: name.into(),
            available,
            version: if available { "Installed" } else { "Missing" }.into(),
            detail: "Aircrack-ng distribution package".into(),
        });
    }
    let installed = std::path::Path::new(crate::capture::HELPER).is_file();
    result.tools.push(ToolStatus {
        name: "Capture helper".into(),
        available: installed,
        version: if installed {
            env!("CARGO_PKG_VERSION")
        } else {
            "—"
        }
        .into(),
        detail: if installed {
            "Scoped polkit helper installed; hardware preflight runs at capture start"
        } else {
            "Run scripts/install-helper.sh after building the helper"
        }
        .into(),
    });
    match crate::network::adapters().await {
        Ok(adapters) => result.adapters = adapters,
        Err(error) => result.errors.push(error),
    }
    #[cfg(target_os = "linux")]
    match zbus::Connection::system().await {
        Ok(connection) => {
            let probe = zbus::Proxy::new(
                &connection,
                "org.freedesktop.NetworkManager",
                "/org/freedesktop/NetworkManager",
                "org.freedesktop.NetworkManager",
            )
            .await;
            if let Ok(proxy) = probe {
                if let Err(error) = proxy.get_property::<String>("Version").await {
                    result
                        .errors
                        .push(format!("NetworkManager service: {error}"))
                }
            }
        }
        Err(error) => result
            .errors
            .push(format!("System D-Bus unavailable: {error}")),
    }
    result
}
pub async fn kismet_check(token: String) -> Result<String, String> {
    let token = zeroize::Zeroizing::new(token);
    if token.is_empty()
        || token.len() > 256
        || !token
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err("Invalid Kismet API token".into());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Cannot initialize local HTTP client")?;
    let response = client
        .get("http://127.0.0.1:2501/system/status.json")
        .query(&[("KISMET", token.as_str())])
        .send()
        .await
        .map_err(|_| "Local Kismet is unavailable on 127.0.0.1:2501")?;
    if !response.status().is_success() {
        return Err(format!(
            "Kismet authentication/status request returned {}",
            response.status()
        ));
    }
    if response.content_length().is_some_and(|n| n > 262144) {
        return Err("Kismet status response exceeds limit".into());
    }
    let mut response = response;
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Kismet response read failed")?
    {
        if bytes.len() + chunk.len() > 262144 {
            return Err("Kismet response limit exceeded".into());
        }
        bytes.extend(chunk)
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| "Kismet returned an unsupported status format")?;
    let version = value
        .get("kismet.system.version")
        .and_then(|v| v.as_str())
        .unwrap_or("version not exposed");
    Ok(format!(
        "Authenticated local Kismet status available: {version}. Telemetry import is not enabled."
    ))
}
