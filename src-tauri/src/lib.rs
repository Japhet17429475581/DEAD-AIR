pub mod aircrack;
pub mod broker;
pub mod capture;
pub mod capture_adapter;
#[cfg(feature = "desktop")]
mod commands;
pub mod diagnostics;
pub mod lab;
pub mod mac;
pub mod model;
pub mod network;
pub mod packets;
pub mod pmkid;
pub mod privilege;
pub mod process;
pub mod sessions;
pub mod setup;
pub mod storage;

#[cfg(feature = "desktop")]
pub fn run() {
    #[cfg(target_os = "linux")]
    if unsafe { libc::geteuid() } == 0 {
        eprintln!("Run the DEAD//AIR GUI as a regular user. Only the scoped polkit helper may be elevated.");
        std::process::exit(1);
    }
    use tauri::Manager;
    tauri::Builder::default()
        .setup(|app| {
            let root = app.path().app_data_dir()?.join("sessions");
            app.manage(commands::Backend::new(root));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::open_reference,
            commands::privileged_status,
            commands::privileged_authorize,
            commands::mac_status,
            commands::mac_start,
            commands::mac_restore,
            commands::mac_recover,
            commands::panic_cleanup,
            commands::system_check,
            commands::adapter_report,
            commands::recover_capture,
            commands::dependency_plan,
            commands::install_dependencies,
            commands::discover,
            commands::cancel_discovery,
            commands::session_start,
            commands::session_status,
            commands::session_end,
            commands::session_recover,
            commands::session_purge,
            commands::report_export,
            commands::capture_start,
            commands::capture_status,
            commands::capture_stop,
            commands::capture_export,
            commands::hashcat_devices,
            commands::lab_start,
            commands::lab_status,
            commands::lab_stop,
            commands::kismet_check
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let handle = window.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    let backend = handle.state::<commands::Backend>();
                    match backend.close().await {
                        Ok(()) => handle.exit(0),
                        Err(error) => {
                            use tauri::Emitter;
                            let _ = handle.emit("shutdown-error", error);
                        }
                    }
                });
            }
        })
        .run(tauri::generate_context!())
        .expect("Unable to initialize DEAD//AIR runtime");
}
