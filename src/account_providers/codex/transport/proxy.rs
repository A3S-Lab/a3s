//! Route-aware proxy resolution for Codex traffic.

use reqwest::Url;

#[cfg(target_os = "macos")]
mod macos;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ProxyRoute {
    Direct,
    Http(Url),
    Socks(Url),
    Unsupported(String),
}

pub(super) fn resolve(target: &Url) -> ProxyRoute {
    if bypasses_proxy(target) {
        return ProxyRoute::Direct;
    }
    if let Some(proxy) = environment_proxy(target) {
        return proxy;
    }
    #[cfg(target_os = "macos")]
    if let Some(route) = macos::resolve(target.as_str()) {
        return route;
    }
    ProxyRoute::Direct
}

fn environment_proxy(target: &Url) -> Option<ProxyRoute> {
    let keys: &[&str] = match target.scheme() {
        "https" | "wss" => &["https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"],
        _ => &["http_proxy", "HTTP_PROXY", "all_proxy", "ALL_PROXY"],
    };
    keys.iter().find_map(|key| {
        std::env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(|value| parse_proxy(&value))
    })
}

fn parse_proxy(value: &str) -> ProxyRoute {
    match Url::parse(value) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => ProxyRoute::Http(url),
        Ok(url) if matches!(url.scheme(), "socks5" | "socks5h") => ProxyRoute::Socks(url),
        Ok(url) => ProxyRoute::Unsupported(url.scheme().to_string()),
        Err(_) => ProxyRoute::Unsupported("invalid".to_string()),
    }
}

fn bypasses_proxy(target: &Url) -> bool {
    let Some(host) = target.host_str() else {
        return true;
    };
    if matches!(host, "localhost" | "127.0.0.1" | "::1") {
        return true;
    }
    let no_proxy = std::env::var("no_proxy")
        .or_else(|_| std::env::var("NO_PROXY"))
        .unwrap_or_default();
    host_matches_no_proxy(host, target.port_or_known_default(), &no_proxy)
}

fn host_matches_no_proxy(host: &str, port: Option<u16>, no_proxy: &str) -> bool {
    no_proxy.split(',').map(str::trim).any(|entry| {
        if entry.is_empty() {
            return false;
        }
        if entry == "*" {
            return true;
        }
        let (candidate, candidate_port) =
            entry
                .rsplit_once(':')
                .map_or((entry, None), |(host, port)| match port.parse::<u16>() {
                    Ok(port) => (host, Some(port)),
                    Err(_) => (entry, None),
                });
        if candidate_port.is_some() && candidate_port != port {
            return false;
        }
        let candidate = candidate.trim_start_matches('.');
        host.eq_ignore_ascii_case(candidate)
            || host
                .to_ascii_lowercase()
                .ends_with(&format!(".{}", candidate.to_ascii_lowercase()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_proxy_matches_domains_and_ports() {
        assert!(host_matches_no_proxy(
            "api.example.com",
            Some(443),
            ".example.com"
        ));
        assert!(host_matches_no_proxy(
            "chatgpt.com",
            Some(443),
            "chatgpt.com:443"
        ));
        assert!(!host_matches_no_proxy(
            "chatgpt.com",
            Some(443),
            "chatgpt.com:80"
        ));
    }

    #[test]
    fn recognizes_socks_and_rejects_unknown_proxy_schemes() {
        assert_eq!(
            parse_proxy("socks5://127.0.0.1:1080"),
            ProxyRoute::Socks(Url::parse("socks5://127.0.0.1:1080").unwrap())
        );
        assert_eq!(
            parse_proxy("ftp://127.0.0.1:21"),
            ProxyRoute::Unsupported("ftp".to_string())
        );
    }
}
