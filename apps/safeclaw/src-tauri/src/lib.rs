mod browser;
mod power;
mod server;
mod voice;

use tauri::Manager;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const MIN_WINDOW_WIDTH: f64 = 800.0;
const MIN_WINDOW_HEIGHT: f64 = 600.0;

fn startup_window_size(screen_width: f64, screen_height: f64) -> (f64, f64) {
    let target_width = screen_width * 0.82;
    let target_height = screen_height * 0.86;
    let max_width = (screen_width - 64.0).max(MIN_WINDOW_WIDTH);
    let max_height = (screen_height - 64.0).max(MIN_WINDOW_HEIGHT);

    let width = target_width.clamp(MIN_WINDOW_WIDTH, max_width).round();
    let height = target_height.clamp(MIN_WINDOW_HEIGHT, max_height).round();
    (width, height)
}

#[tauri::command]
fn get_gateway_url() -> String {
    std::env::var("SAFECLAW_GATEWAY_URL").unwrap_or_else(|_| "http://127.0.0.1:18790".to_string())
}

#[tauri::command]
fn get_power_url() -> String {
    std::env::var("SAFECLAW_POWER_URL").unwrap_or_else(|_| power::local_power_base_url())
}

#[tauri::command]
fn get_power_runtime_status() -> power::PowerRuntimeStatus {
    power::embedded_runtime_status()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging — attach the LogBufferLayer so that Power server logs
    // are captured from startup and can be streamed via GET /v1/logs.
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "safeclaw=debug,a3s_code=debug,a3s_power=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .with(power::log_buffer_layer())
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_gateway_url,
            get_power_url,
            get_power_runtime_status,
            voice::voice_tts_status,
            voice::voice_tts_download,
            voice::voice_tts_speak,
            voice::voice_tts_stop,
            browser::browser_open,
            browser::browser_navigate,
            browser::browser_close,
            browser::browser_resize,
            browser::browser_show,
            browser::browser_hide,
            browser::browser_eval,
            browser::browser_go_back,
            browser::browser_go_forward,
            browser::browser_reload,
            browser::browser_get_page_text,
            browser::browser_page_event,
            browser::browser_hide_all,
            browser::browser_show_active,
        ])
        .setup(|app| {
            // Initialize voice state
            app.manage(voice::VoiceState::default());
            // Initialize browser state
            app.manage(browser::BrowserState::default());

            // Size window based on current monitor dimensions.
            let window = app
                .get_webview_window("main")
                .expect("main window should exist");
            if let Some(monitor) = window.current_monitor().ok().flatten() {
                let screen_width = monitor.size().width as f64 / monitor.scale_factor();
                let screen_height = monitor.size().height as f64 / monitor.scale_factor();
                let (win_width, win_height) = startup_window_size(screen_width, screen_height);
                let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize {
                    width: win_width,
                    height: win_height,
                }));
                let _ = window.center();
            }

            #[cfg(debug_assertions)]
            {
                window.open_devtools();
            }

            // Spawn embedded SafeClaw gateway in background
            tauri::async_runtime::spawn(async {
                if let Err(e) = server::start_embedded_gateway().await {
                    tracing::error!("Embedded gateway failed: {e:#}");
                }
            });

            // Spawn embedded local Power inference server in background
            tauri::async_runtime::spawn(async {
                if let Err(e) = power::start_embedded_power().await {
                    tracing::error!("Embedded Power server failed: {e:#}");
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running SafeClaw");
}

#[cfg(test)]
mod tests {
    use super::startup_window_size;

    #[test]
    fn startup_window_scales_for_large_screens() {
        let (w, h) = startup_window_size(2560.0, 1440.0);
        assert_eq!(w, 2099.0);
        assert_eq!(h, 1238.0);
    }

    #[test]
    fn startup_window_respects_minimum_size_on_small_screens() {
        let (w, h) = startup_window_size(1024.0, 640.0);
        assert_eq!(w, 840.0);
        assert_eq!(h, 600.0);
    }

    #[test]
    fn startup_window_never_exceeds_monitor_bounds() {
        let (w, h) = startup_window_size(820.0, 620.0);
        assert_eq!(w, 800.0);
        assert_eq!(h, 600.0);
    }
}
