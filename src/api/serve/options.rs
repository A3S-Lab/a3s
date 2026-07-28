use std::net::{SocketAddr, ToSocketAddrs};
use std::path::PathBuf;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 29653;

#[derive(Clone, Debug)]
pub(crate) struct ServeOptions {
    pub(crate) addr: SocketAddr,
    pub(crate) workspace: PathBuf,
    pub(crate) config_path: Option<PathBuf>,
    pub(crate) web_dir: Option<PathBuf>,
    pub(crate) api_only: bool,
    pub(crate) background: bool,
    pub(crate) replace: bool,
    pub(crate) offline: bool,
    pub(crate) allow_asset_download: bool,
    pub(crate) help: bool,
}

impl ServeOptions {
    pub(super) fn parse(args: &[String]) -> anyhow::Result<Self> {
        let mut host = std::env::var("A3S_CODE_WEB_HOST").unwrap_or_else(|_| DEFAULT_HOST.into());
        let mut port = std::env::var("A3S_CODE_WEB_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(DEFAULT_PORT);
        let mut workspace = std::env::current_dir()?;
        let mut config_path = None;
        let mut web_dir = std::env::var_os("A3S_CODE_WEB_DIR").map(PathBuf::from);
        let mut api_only = false;
        let mut background = false;
        let mut replace = false;
        let offline = environment_flag("A3S_OFFLINE");
        let allow_asset_download = !environment_flag("A3S_NO_AUTO_INSTALL");
        let mut help = false;

        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "-h" | "--help" | "help" => {
                    help = true;
                    index += 1;
                }
                "--host" => {
                    host = take_value(args, &mut index, "--host")?;
                }
                "--port" => {
                    let value = take_value(args, &mut index, "--port")?;
                    port = value
                        .parse::<u16>()
                        .map_err(|_| anyhow::anyhow!("--port must be a number from 0 to 65535"))?;
                }
                "--workspace" | "-w" => {
                    workspace = PathBuf::from(take_value(args, &mut index, "--workspace")?);
                }
                "--config" => {
                    config_path = Some(PathBuf::from(take_value(args, &mut index, "--config")?));
                }
                "--web-dir" => {
                    web_dir = Some(PathBuf::from(take_value(args, &mut index, "--web-dir")?));
                }
                "--api-only" => {
                    api_only = true;
                    index += 1;
                }
                "-d" | "--detach" => {
                    background = true;
                    index += 1;
                }
                "--replace" => {
                    replace = true;
                    index += 1;
                }
                other => anyhow::bail!("unknown a3s web option `{other}`"),
            }
        }

        let addr = resolve_addr(&host, port)?;
        Ok(Self {
            addr,
            workspace,
            config_path,
            web_dir,
            api_only,
            background,
            replace,
            offline,
            allow_asset_download,
            help,
        })
    }
}

fn environment_flag(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .is_some_and(|value| matches!(value.trim(), "1" | "true" | "yes" | "on"))
}

fn take_value(args: &[String], index: &mut usize, flag: &str) -> anyhow::Result<String> {
    let value_index = *index + 1;
    let value = args
        .get(value_index)
        .filter(|value| !value.starts_with('-'))
        .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))?;
    *index += 2;
    Ok(value.clone())
}

fn resolve_addr(host: &str, port: u16) -> anyhow::Result<SocketAddr> {
    let mut addrs = format!("{host}:{port}")
        .to_socket_addrs()
        .map_err(|e| anyhow::anyhow!("invalid host/port {host}:{port}: {e}"))?;
    addrs
        .next()
        .ok_or_else(|| anyhow::anyhow!("could not resolve {host}:{port}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_background_mode_with_explicit_network_options() {
        let options = ServeOptions::parse(&[
            "-d".into(),
            "--host".into(),
            "127.0.0.1".into(),
            "--port".into(),
            "0".into(),
        ])
        .expect("valid web options");

        assert!(options.background);
        assert!(!options.replace);
        assert_eq!(options.addr.ip().to_string(), "127.0.0.1");
        assert_eq!(options.addr.port(), 0);
    }

    #[test]
    fn parses_explicit_safe_replacement() {
        let options = ServeOptions::parse(&["--replace".into()]).expect("valid replacement option");

        assert!(options.replace);
    }

    #[test]
    fn reports_unknown_options_for_the_top_level_web_command() {
        let error = ServeOptions::parse(&["--unknown".into()]).expect_err("unknown option");
        assert!(error.to_string().contains("unknown a3s web option"));
    }
}
