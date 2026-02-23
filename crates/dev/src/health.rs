use std::net::SocketAddr;
use std::time::Duration;

use async_trait::async_trait;
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::config::{HealthConfig, HealthKind, ServiceDef};

#[async_trait]
pub trait HealthProbe: Send + Sync {
    async fn check(&self, port: u16, svc: &ServiceDef) -> bool;
}

pub struct HttpProbe {
    client: reqwest::Client,
}

impl HttpProbe {
    pub fn new(request_timeout: Duration) -> Self {
        // reqwest::Client::builder().build() only fails on TLS init errors,
        // which are unrecoverable at runtime — treat as fatal.
        let client = reqwest::Client::builder()
            .timeout(request_timeout)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client }
    }
}

#[async_trait]
impl HealthProbe for HttpProbe {
    async fn check(&self, port: u16, svc: &ServiceDef) -> bool {
        let path = svc
            .health
            .as_ref()
            .and_then(|h| h.path.as_deref())
            .unwrap_or("/health");
        let url = format!("http://127.0.0.1:{port}{path}");
        self.client
            .get(&url)
            .send()
            .await
            .is_ok_and(|r| r.status().is_success())
    }
}

pub struct TcpProbe {
    timeout: Duration,
}

impl TcpProbe {
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }
}

#[async_trait]
impl HealthProbe for TcpProbe {
    async fn check(&self, port: u16, _svc: &ServiceDef) -> bool {
        let Ok(addr) = format!("127.0.0.1:{port}").parse::<SocketAddr>() else {
            return false;
        };
        timeout(self.timeout, TcpStream::connect(addr))
            .await
            .is_ok_and(|r| r.is_ok())
    }
}

pub struct HealthChecker {
    probe: Box<dyn HealthProbe>,
    pub config: HealthConfig,
}

impl HealthChecker {
    pub fn for_service(svc: &ServiceDef) -> Option<Self> {
        let config = svc.health.clone()?;
        let probe: Box<dyn HealthProbe> = match config.kind {
            HealthKind::Http => Box::new(HttpProbe::new(config.timeout)),
            HealthKind::Tcp => Box::new(TcpProbe::new(config.timeout)),
        };
        Some(Self { probe, config })
    }

    /// Poll until healthy or retries exhausted. Returns true if healthy.
    /// `port` is the actual bound port (may differ from svc.port for port=0 services).
    pub async fn wait_healthy(&self, svc: &ServiceDef, port: u16) -> bool {
        for _ in 0..self.config.retries {
            tokio::time::sleep(self.config.interval).await;
            if self.probe.check(port, svc).await {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    fn dummy_svc() -> ServiceDef {
        ServiceDef {
            cmd: "echo".into(),
            dir: None,
            port: 0,
            subdomain: None,
            env: Default::default(),
            depends_on: vec![],
            watch: None,
            health: None,
        }
    }

    async fn free_port() -> u16 {
        TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    #[tokio::test]
    async fn test_tcp_probe_success() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        // Keep listener alive in background
        tokio::spawn(async move {
            let _ = listener.accept().await;
        });

        let probe = TcpProbe::new(Duration::from_secs(1));
        let svc = dummy_svc();
        assert!(probe.check(port, &svc).await);
    }

    #[tokio::test]
    async fn test_tcp_probe_failure() {
        let port = free_port().await; // bind then drop — port is closed
        let probe = TcpProbe::new(Duration::from_millis(100));
        let svc = dummy_svc();
        assert!(!probe.check(port, &svc).await);
    }

    #[tokio::test]
    async fn test_wait_healthy_succeeds_on_first_retry() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let _ = listener.accept().await;
            }
        });

        use crate::config::{HealthConfig, HealthKind};
        let config = HealthConfig {
            kind: HealthKind::Tcp,
            path: None,
            interval: Duration::from_millis(10),
            timeout: Duration::from_millis(200),
            retries: 3,
        };
        let checker = HealthChecker {
            probe: Box::new(TcpProbe::new(config.timeout)),
            config,
        };
        let svc = dummy_svc();
        assert!(checker.wait_healthy(&svc, port).await);
    }

    #[tokio::test]
    async fn test_wait_healthy_exhausts_retries() {
        let port = free_port().await;

        use crate::config::{HealthConfig, HealthKind};
        let config = HealthConfig {
            kind: HealthKind::Tcp,
            path: None,
            interval: Duration::from_millis(1),
            timeout: Duration::from_millis(50),
            retries: 2,
        };
        let checker = HealthChecker {
            probe: Box::new(TcpProbe::new(config.timeout)),
            config,
        };
        let svc = dummy_svc();
        assert!(!checker.wait_healthy(&svc, port).await);
    }
}
