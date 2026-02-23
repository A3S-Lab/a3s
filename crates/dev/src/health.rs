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
        Self {
            client: reqwest::Client::builder()
                .timeout(request_timeout)
                .build()
                .expect("failed to build health check HTTP client"),
        }
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
        let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
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
