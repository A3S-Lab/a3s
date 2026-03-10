use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl};

#[derive(Default)]
pub struct BrowserState {
    pub tabs: Mutex<HashMap<String, String>>,
}

fn webview_label(tab_id: &str) -> String {
    format!("browser-tab-{tab_id}")
}

/// Copilot initialization script injected into every page
const COPILOT_SCRIPT: &str = r#"
(function() {
  if (window.__safeclaw_injected) return;
  window.__safeclaw_injected = true;

  // Notify Rust when page title changes
  const observer = new MutationObserver(() => {
    window.__TAURI_INTERNALS__?.invoke('browser_page_event', {
      event: 'title_changed',
      data: document.title
    }).catch(() => {});
  });
  observer.observe(document.querySelector('title') || document.head, {
    subtree: true, childList: true, characterData: true
  });

  // Expose helper for copilot to extract page content
  window.__safeclaw = {
    getPageText: () => document.body?.innerText ?? '',
    getPageHtml: () => document.documentElement?.outerHTML ?? '',
    getSelection: () => window.getSelection()?.toString() ?? '',
    getUrl: () => window.location.href,
    getTitle: () => document.title,
    scrollTo: (x, y) => window.scrollTo(x, y),
    click: (selector) => document.querySelector(selector)?.click(),
    fill: (selector, value) => {
      const el = document.querySelector(selector);
      if (el) { el.value = value; el.dispatchEvent(new Event('input', {bubbles: true})); }
    },
  };
})();
"#;

#[tauri::command]
pub async fn browser_open(
    app: AppHandle,
    state: tauri::State<'_, BrowserState>,
    tab_id: String,
    url: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let label = webview_label(&tab_id);

    // Close existing webview for this tab if any
    {
        let tabs = state.tabs.lock().unwrap();
        if tabs.contains_key(&tab_id) {
            if let Some(existing) = app.get_webview_window(&label) {
                let _ = existing.close();
            }
        }
    }

    let window = app.get_window("main").ok_or("main window not found")?;

    let webview = tauri::webview::WebviewBuilder::new(
        &label,
        WebviewUrl::External(url.parse().map_err(|e| format!("invalid url: {e}"))?),
    )
    .initialization_script(COPILOT_SCRIPT)
    .auto_resize();

    window
        .add_child(
            webview,
            LogicalPosition::new(x, y),
            LogicalSize::new(width, height),
        )
        .map_err(|e| format!("failed to create webview: {e}"))?;

    state.tabs.lock().unwrap().insert(tab_id, label);
    Ok(())
}

#[tauri::command]
pub async fn browser_navigate(
    app: AppHandle,
    _state: tauri::State<'_, BrowserState>,
    tab_id: String,
    url: String,
) -> Result<(), String> {
    let label = webview_label(&tab_id);
    let webview = app
        .get_webview(&label)
        .ok_or_else(|| format!("webview {label} not found"))?;
    webview
        .navigate(url.parse().map_err(|e| format!("invalid url: {e}"))?)
        .map_err(|e| format!("navigate failed: {e}"))
}

#[tauri::command]
pub async fn browser_close(
    app: AppHandle,
    state: tauri::State<'_, BrowserState>,
    tab_id: String,
) -> Result<(), String> {
    let label = webview_label(&tab_id);
    if let Some(webview) = app.get_webview(&label) {
        webview.close().map_err(|e| format!("close failed: {e}"))?;
    }
    state.tabs.lock().unwrap().remove(&tab_id);
    Ok(())
}

#[tauri::command]
pub async fn browser_resize(
    app: AppHandle,
    tab_id: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let label = webview_label(&tab_id);
    let webview = app
        .get_webview(&label)
        .ok_or_else(|| format!("webview {label} not found"))?;
    webview
        .set_position(LogicalPosition::new(x, y))
        .map_err(|e| format!("set_position failed: {e}"))?;
    webview
        .set_size(LogicalSize::new(width, height))
        .map_err(|e| format!("set_size failed: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn browser_show(app: AppHandle, tab_id: String) -> Result<(), String> {
    let label = webview_label(&tab_id);
    if let Some(webview) = app.get_webview(&label) {
        webview.show().map_err(|e| format!("show failed: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn browser_hide(app: AppHandle, tab_id: String) -> Result<(), String> {
    let label = webview_label(&tab_id);
    if let Some(webview) = app.get_webview(&label) {
        webview.hide().map_err(|e| format!("hide failed: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn browser_eval(app: AppHandle, tab_id: String, script: String) -> Result<(), String> {
    let label = webview_label(&tab_id);
    let webview = app
        .get_webview(&label)
        .ok_or_else(|| format!("webview {label} not found"))?;
    webview
        .eval(&script)
        .map_err(|e| format!("eval failed: {e}"))
}

#[tauri::command]
pub async fn browser_go_back(app: AppHandle, tab_id: String) -> Result<(), String> {
    let label = webview_label(&tab_id);
    let webview = app
        .get_webview(&label)
        .ok_or_else(|| format!("webview {label} not found"))?;
    webview
        .eval("window.history.back()")
        .map_err(|e| format!("eval failed: {e}"))
}

#[tauri::command]
pub async fn browser_go_forward(app: AppHandle, tab_id: String) -> Result<(), String> {
    let label = webview_label(&tab_id);
    let webview = app
        .get_webview(&label)
        .ok_or_else(|| format!("webview {label} not found"))?;
    webview
        .eval("window.history.forward()")
        .map_err(|e| format!("eval failed: {e}"))
}

#[tauri::command]
pub async fn browser_reload(app: AppHandle, tab_id: String) -> Result<(), String> {
    let label = webview_label(&tab_id);
    let webview = app
        .get_webview(&label)
        .ok_or_else(|| format!("webview {label} not found"))?;
    webview
        .eval("window.location.reload()")
        .map_err(|e| format!("eval failed: {e}"))
}

/// Get page content via injected helper
#[tauri::command]
pub async fn browser_get_page_text(app: AppHandle, tab_id: String) -> Result<String, String> {
    let label = webview_label(&tab_id);
    let webview = app
        .get_webview(&label)
        .ok_or_else(|| format!("webview {label} not found"))?;
    // eval returns () in Tauri v2; use a workaround via IPC event
    webview
        .eval("window.__safeclaw?.getPageText() ?? ''")
        .map_err(|e| format!("eval failed: {e}"))?;
    Ok(String::new())
}

#[tauri::command]
pub async fn browser_hide_all(
    app: AppHandle,
    state: tauri::State<'_, BrowserState>,
) -> Result<(), String> {
    let tabs = state.tabs.lock().unwrap().clone();
    for label in tabs.values() {
        if let Some(webview) = app.get_webview(label) {
            let _ = webview.hide();
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn browser_show_active(
    app: AppHandle,
    state: tauri::State<'_, BrowserState>,
    tab_id: String,
) -> Result<(), String> {
    let label = webview_label(&tab_id);
    // Hide all others first
    let tabs = state.tabs.lock().unwrap().clone();
    for (id, lbl) in &tabs {
        if *id != tab_id {
            if let Some(webview) = app.get_webview(lbl) {
                let _ = webview.hide();
            }
        }
    }
    // Show active
    if let Some(webview) = app.get_webview(&label) {
        webview.show().map_err(|e| format!("show failed: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn browser_page_event(
    _app: AppHandle,
    event: String,
    data: String,
) -> Result<(), String> {
    tracing::debug!("browser page event: {event} = {data}");
    Ok(())
}
