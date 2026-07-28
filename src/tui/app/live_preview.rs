//! Host-owned live-preview lifecycle for `/preview` and `/ide` shortcuts.

use super::*;
use std::net::{IpAddr, SocketAddr};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum LivePreviewCommand {
    Open(String),
    Status,
    Stop,
}

pub(super) struct LivePreviewLaunch {
    pub(super) target: String,
    pub(super) url: String,
    pub(super) window: remote_ui::LivePreviewWindow,
}

pub(super) struct LivePreviewState {
    pub(super) target: String,
    pub(super) url: String,
    pub(super) window: remote_ui::LivePreviewWindow,
}

pub(super) fn parse_live_preview_command(rest: &str) -> Result<LivePreviewCommand, &'static str> {
    let target = strip_matching_quotes(rest.trim());
    match target {
        "" => Err("usage: /preview <path|localhost-url> | status | stop"),
        "status" => Ok(LivePreviewCommand::Status),
        "stop" => Ok(LivePreviewCommand::Stop),
        _ => Ok(LivePreviewCommand::Open(normalize_live_preview_target(
            target,
        ))),
    }
}

fn strip_matching_quotes(value: &str) -> &str {
    if value.len() < 2 {
        return value;
    }
    let bytes = value.as_bytes();
    if matches!(
        (bytes.first(), bytes.last()),
        (Some(b'"'), Some(b'"')) | (Some(b'\''), Some(b'\''))
    ) {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

fn normalize_live_preview_target(target: &str) -> String {
    if target.contains(char::is_whitespace) {
        return target.to_string();
    }
    let candidate = format!("http://{target}");
    if url::Url::parse(&candidate)
        .ok()
        .and_then(|url| url.host_str().map(is_loopback_host))
        .unwrap_or(false)
    {
        candidate
    } else {
        target.to_string()
    }
}

fn is_loopback_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    host == "localhost"
        || host.ends_with(".localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn preview_server_args(workspace: &Path, config_path: &Path) -> Result<Vec<String>, String> {
    let workspace = workspace
        .to_str()
        .ok_or_else(|| "preview workspace must be valid UTF-8".to_string())?;
    let config_path = config_path
        .to_str()
        .ok_or_else(|| "preview config path must be valid UTF-8".to_string())?;
    Ok(vec![
        "--detach".to_string(),
        "--host".to_string(),
        "127.0.0.1".to_string(),
        // Reuse an existing managed instance for this workspace. A fresh
        // preview server receives an ephemeral port so an unrelated service on
        // the product default can never be replaced.
        "--port".to_string(),
        "0".to_string(),
        "--workspace".to_string(),
        workspace.to_string(),
        "--config".to_string(),
        config_path.to_string(),
    ])
}

fn live_preview_url(address: SocketAddr, target: &str) -> Result<String, String> {
    let mut url = url::Url::parse(&format!("http://{address}/"))
        .map_err(|error| format!("could not build the A3S Web URL: {error}"))?;
    url.query_pairs_mut().append_pair("preview", target);
    url.set_fragment(Some("home"));
    Ok(url.into())
}

fn environment_flag(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .is_some_and(|value| matches!(value.trim(), "1" | "true" | "yes" | "on"))
}

async fn launch_live_preview(
    workspace: PathBuf,
    config_path: PathBuf,
    target: String,
    preferred_webview: Option<PathBuf>,
) -> Result<LivePreviewLaunch, String> {
    let args = preview_server_args(&workspace, &config_path)?;
    let cancellation = tokio_util::sync::CancellationToken::new();
    let outcome = crate::api::run_web(
        &args,
        &cancellation,
        environment_flag("A3S_OFFLINE"),
        !environment_flag("A3S_NO_AUTO_INSTALL"),
    )
    .await
    .map_err(|error| format!("could not start A3S Web: {error:#}"))?;

    let (address, api_only) = match outcome {
        crate::api::ServeOutcome::Detached { instance, .. } => {
            (instance.address, instance.api_only)
        }
        crate::api::ServeOutcome::Existing(instance) => {
            (instance.address, instance.api_only.unwrap_or(false))
        }
        crate::api::ServeOutcome::Help | crate::api::ServeOutcome::ForegroundStopped => {
            return Err("A3S Web stopped before live preview was ready".to_string())
        }
    };
    if api_only {
        return Err(format!(
            "A3S Web at http://{address}/ is API-only; stop that instance before opening live preview"
        ));
    }

    let url = live_preview_url(address, &target)?;
    let window = remote_ui::open_live_preview_window_with(&url, preferred_webview.as_deref())
        .map_err(|error| format!("could not open the live-preview window: {error}"))?;
    Ok(LivePreviewLaunch {
        target,
        url,
        window,
    })
}

impl App {
    pub(super) fn submit_live_preview_command(&mut self, rest: &str) -> Option<Cmd<Msg>> {
        self.textarea.clear();
        let command = match parse_live_preview_command(rest) {
            Ok(command) => command,
            Err(usage) => {
                self.push_line(&Style::new().fg(TN_YELLOW).render(&format!("  {usage}")));
                return None;
            }
        };

        match command {
            LivePreviewCommand::Status => {
                self.show_live_preview_status();
                None
            }
            LivePreviewCommand::Stop => {
                self.preview_launch_seq = self.preview_launch_seq.wrapping_add(1);
                let pending = self.live_preview_pending.take();
                let launching = pending.is_some();
                if let Some((_, _, status_entry)) = pending {
                    self.replace_tracked_line(
                        status_entry,
                        &Style::new()
                            .fg(TN_YELLOW)
                            .render("  live-preview launch cancelled"),
                    );
                }
                let previous = self.live_preview.take();
                let had_preview = previous.is_some();
                let browser = previous.as_ref().is_some_and(|state| {
                    state.window.opened_with() == remote_ui::OpenedWith::Browser
                });
                drop(previous);
                let text = if browser {
                    "  ✓ stopped tracking live preview · the browser tab remains open · A3S Web remains running"
                } else if had_preview {
                    "  ✓ stopped live preview · A3S Web remains running"
                } else if launching {
                    "  ✓ cancelled the pending live preview · A3S Web remains running"
                } else {
                    "  live preview is not running"
                };
                self.push_line(&Style::new().fg(TN_GRAY).render(text));
                None
            }
            LivePreviewCommand::Open(target) => {
                if let Some((_, _, status_entry)) = self.live_preview_pending.take() {
                    self.replace_tracked_line(
                        status_entry,
                        &Style::new()
                            .fg(TN_YELLOW)
                            .render("  live-preview launch replaced by a newer request"),
                    );
                }
                self.preview_launch_seq = self.preview_launch_seq.wrapping_add(1);
                let request_id = self.preview_launch_seq;
                let status_entry = self.push_tracked_line(
                    &Style::new()
                        .fg(TN_GRAY)
                        .render(&format!("  opening live preview for {target}…")),
                );
                self.live_preview_pending = Some((request_id, target.clone(), status_entry));
                let workspace = PathBuf::from(&self.cwd);
                let config_path = self.config_path.clone();
                let preferred_webview = self.agent_presence.webview_binary().map(Path::to_path_buf);
                Some(cmd::cmd(move || async move {
                    Msg::LivePreviewLaunched {
                        request_id,
                        status_entry,
                        result: Box::new(
                            launch_live_preview(workspace, config_path, target, preferred_webview)
                                .await,
                        ),
                    }
                }))
            }
        }
    }

    pub(super) fn apply_live_preview_launch(
        &mut self,
        request_id: u64,
        status_entry: TranscriptEntryId,
        result: Result<LivePreviewLaunch, String>,
    ) {
        if self
            .live_preview_pending
            .as_ref()
            .map(|(pending_id, _, _)| *pending_id)
            != Some(request_id)
        {
            // Dropping a stale successful result closes its tracked native
            // window through `LivePreviewWindow::drop`.
            return;
        }
        self.live_preview_pending = None;
        match result {
            Ok(launch) => {
                let opened_with = launch.window.opened_with();
                let target = launch.target.clone();
                let url = launch.url.clone();
                self.live_preview = Some(LivePreviewState {
                    target,
                    url: url.clone(),
                    window: launch.window,
                });
                let surface = match opened_with {
                    remote_ui::OpenedWith::Webview => "native window",
                    remote_ui::OpenedWith::Browser => "browser fallback",
                };
                self.replace_tracked_line(
                    status_entry,
                    &Style::new()
                        .fg(TN_GREEN)
                        .render(&format!("  ✓ live preview opened in {surface} · {url}")),
                );
            }
            Err(error) => {
                self.replace_tracked_line(
                    status_entry,
                    &Style::new()
                        .fg(TN_RED)
                        .render(&format!("  live preview failed: {error}")),
                );
            }
        }
    }

    fn show_live_preview_status(&mut self) {
        if let Some((_, target, _)) = self.live_preview_pending.as_ref() {
            self.push_line(
                &Style::new()
                    .fg(TN_GRAY)
                    .render(&format!("  live preview is starting · {target}")),
            );
            return;
        }

        let Some(state) = self.live_preview.as_mut() else {
            self.push_line(
                &Style::new()
                    .fg(TN_GRAY)
                    .render("  live preview is not running"),
            );
            return;
        };
        let target = state.target.clone();
        let url = state.url.clone();
        let status = state.window.webview_running();
        match status {
            Ok(Some(true)) => self.push_line(
                &Style::new()
                    .fg(TN_GREEN)
                    .render(&format!("  ✓ live preview is running · {target} · {url}")),
            ),
            Ok(None) => self.push_line(&Style::new().fg(TN_GREEN).render(&format!(
                "  ✓ live preview is open in the browser · {target} · {url}"
            ))),
            Ok(Some(false)) => {
                self.live_preview = None;
                self.push_line(
                    &Style::new()
                        .fg(TN_GRAY)
                        .render("  live-preview window was closed · A3S Web remains running"),
                );
            }
            Err(error) => self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render(&format!("  live-preview status is unavailable: {error}")),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_open_status_stop_and_quoted_paths() {
        assert_eq!(
            parse_live_preview_command("site/index.html"),
            Ok(LivePreviewCommand::Open("site/index.html".to_string()))
        );
        assert_eq!(
            parse_live_preview_command("\"docs/Product brief.pdf\""),
            Ok(LivePreviewCommand::Open(
                "docs/Product brief.pdf".to_string()
            ))
        );
        assert_eq!(
            parse_live_preview_command("status"),
            Ok(LivePreviewCommand::Status)
        );
        assert_eq!(
            parse_live_preview_command("stop"),
            Ok(LivePreviewCommand::Stop)
        );
        assert!(parse_live_preview_command("").is_err());
    }

    #[test]
    fn normalizes_loopback_shorthand_without_rewriting_workspace_paths() {
        assert_eq!(
            parse_live_preview_command("localhost:5173/dashboard"),
            Ok(LivePreviewCommand::Open(
                "http://localhost:5173/dashboard".to_string()
            ))
        );
        assert_eq!(
            parse_live_preview_command("127.0.0.1:4173"),
            Ok(LivePreviewCommand::Open(
                "http://127.0.0.1:4173".to_string()
            ))
        );
        assert_eq!(
            parse_live_preview_command("artifacts/report.html"),
            Ok(LivePreviewCommand::Open(
                "artifacts/report.html".to_string()
            ))
        );
    }

    #[test]
    fn deep_link_percent_encodes_the_target_and_routes_to_home() {
        let address = "127.0.0.1:29653".parse().unwrap();
        let url = live_preview_url(address, "docs/Product brief.pdf").unwrap();

        assert_eq!(
            url,
            "http://127.0.0.1:29653/?preview=docs%2FProduct+brief.pdf#home"
        );
    }

    #[test]
    fn detached_server_uses_an_ephemeral_loopback_port() {
        let args = preview_server_args(Path::new("/workspace"), Path::new("/config.acl")).unwrap();

        assert!(args.windows(2).any(|pair| pair == ["--host", "127.0.0.1"]));
        assert!(args.windows(2).any(|pair| pair == ["--port", "0"]));
        assert!(args.iter().any(|arg| arg == "--detach"));
        assert!(!args.iter().any(|arg| arg == "--replace"));
    }
}
