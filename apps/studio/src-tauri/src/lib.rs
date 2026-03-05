use tauri::Manager;
use tauri_plugin_shell::ShellExt;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const SIDECAR_NAME: &str = "studio-sidecar";

#[tauri::command]
fn get_sidecar_url() -> String {
    std::env::var("STUDIO_SIDECAR_URL").unwrap_or_else(|_| "http://127.0.0.1:3000".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "studio=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_http::init())
        .invoke_handler(tauri::generate_handler![get_sidecar_url])
        .setup(|app| {
            // Size window based on current monitor dimensions.
            let window = app
                .get_webview_window("main")
                .expect("main window should exist");
            if let Some(monitor) = window.current_monitor().ok().flatten() {
                let sf = monitor.scale_factor();
                let sw = monitor.size().width as f64 / sf;
                let sh = monitor.size().height as f64 / sf;
                let w = (sw * 0.82).clamp(900.0, (sw - 64.0).max(900.0)).round();
                let h = (sh * 0.86).clamp(600.0, (sh - 64.0).max(600.0)).round();
                let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize {
                    width: w,
                    height: h,
                }));
                let _ = window.center();
            }

            #[cfg(debug_assertions)]
            {
                window.open_devtools();
                tracing::info!("Development mode: sidecar should be started manually with 'pnpm dev:sidecar'");
            }

            // Spawn NestJS sidecar process (production only)
            #[cfg(not(debug_assertions))]
            {
                let shell = app.shell();
                let (mut _rx, _child) = shell
                    .sidecar(SIDECAR_NAME)
                    .expect("failed to create sidecar command")
                    .spawn()
                    .expect("failed to spawn sidecar");

                tracing::info!("NestJS sidecar started");

                // Log sidecar output in background
                tauri::async_runtime::spawn(async move {
                    use tauri_plugin_shell::process::CommandEvent;
                    while let Some(event) = _rx.recv().await {
                        match event {
                            CommandEvent::Stdout(line) => {
                                tracing::info!("[sidecar] {}", String::from_utf8_lossy(&line));
                            }
                            CommandEvent::Stderr(line) => {
                                tracing::warn!("[sidecar] {}", String::from_utf8_lossy(&line));
                            }
                            CommandEvent::Terminated(payload) => {
                                tracing::info!("[sidecar] terminated: {:?}", payload);
                                break;
                            }
                            CommandEvent::Error(err) => {
                                tracing::error!("[sidecar] error: {}", err);
                            }
                            _ => {}
                        }
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Studio");
}
