use std::net::SocketAddr;
use std::time::Duration;

use async_trait::async_trait;
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::config::{HealthConfig, HealthKind, ServiceDef};

#[async_trait]
pub trait HealthProbe: Send + Sync {
    async fn check(&self, svc: &ServiceDef) -> bool;
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
    async fn check(&self, svc: &ServiceDef) -> bool {
        let path = svc
            .health
            .as_ref()
            .and_then(|h| h.path.as_deref())
            .unwrap_or("/health");
        let url = format!("http://127.0.0.1:{}{}", svc.port, path);
        self.client.get(&url).send().await.map_or(false, |r| r.status().is_success())
    }
}

pub struct TcpProbe;

#[async_trait]
impl HealthProbe for TcpProbe {
    async fn check(&self, svc: &ServiceDef) -> bool {
        let addr: SocketAddr = format!("127.0.0.1:{}", svc.port).parse().unwrap();
        timeout(Duration::from_secs(1), TcpStream::connect(addr))
            .await
            .map_or(false, |r| r.is_ok())
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
            HealthKind::Tcp => Box::new(TcpProbe),
        };
        Some(Self { probe, config })
    }

    /// Poll until healthy or retries exhausted. Returns true if healthy.
    pub async fn wait_healthy(&self, svc: &ServiceDef) -> bool {
        for _ in 0..self.config.retries {
            tokio::time::sleep(self.config.interval).await;
            if self.probe.check(svc).await {
                return true;
            }
        }
        false
    }

    pub async fn check_once(&self, svc: &ServiceDef) -> bool {
        self.probe.check(svc).await
    }
}
