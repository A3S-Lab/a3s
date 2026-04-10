//! Load Balancer - distributes requests across endpoints.
//!
//! Implements various load balancing strategies.

use crate::deployment::registry::Endpoint;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Load balancing strategy.
#[derive(Debug, Clone, Copy)]
pub enum LoadBalancingStrategy {
    /// Round-robin - each endpoint gets equal share in rotation.
    RoundRobin,
    /// Least connections - send to endpoint with fewest active requests.
    LeastConnections,
    /// Random - randomly select an endpoint.
    Random,
    /// IP hash - consistent hashing based on client IP.
    IpHash,
}

/// Load balancer - distributes requests across endpoints.
pub struct LoadBalancer {
    strategy: LoadBalancingStrategy,
    round_robin_counter: std::sync::atomic::AtomicUsize,
}

impl LoadBalancer {
    /// Create a new load balancer with the given strategy.
    pub fn new(strategy: LoadBalancingStrategy) -> Self {
        Self {
            strategy,
            round_robin_counter: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Select an endpoint based on the load balancing strategy.
    pub fn select<'a>(
        &self,
        endpoints: &'a [Endpoint],
        client_ip: Option<IpAddr>,
    ) -> Option<&'a Endpoint> {
        if endpoints.is_empty() {
            return None;
        }

        match self.strategy {
            LoadBalancingStrategy::RoundRobin => self.round_robin(endpoints),
            LoadBalancingStrategy::LeastConnections => self.least_connections(endpoints),
            LoadBalancingStrategy::Random => self.random(endpoints),
            LoadBalancingStrategy::IpHash => self.ip_hash(endpoints, client_ip),
        }
    }

    /// Round-robin selection.
    fn round_robin<'a>(&self, endpoints: &'a [Endpoint]) -> Option<&'a Endpoint> {
        let count = endpoints.len();
        if count == 0 {
            return None;
        }

        let idx = self
            .round_robin_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            % count;
        endpoints.get(idx)
    }

    /// Select endpoint with least connections (lowest load).
    fn least_connections<'a>(&self, endpoints: &'a [Endpoint]) -> Option<&'a Endpoint> {
        endpoints.iter().min_by_key(|e| e.load)
    }

    /// Random selection.
    fn random<'a>(&self, endpoints: &'a [Endpoint]) -> Option<&'a Endpoint> {
        use std::time::Instant;
        let now = Instant::now().elapsed().as_nanos() as usize;
        let idx = now % endpoints.len();
        endpoints.get(idx)
    }

    /// IP hash - consistent hashing based on client IP.
    fn ip_hash<'a>(
        &self,
        endpoints: &'a [Endpoint],
        client_ip: Option<IpAddr>,
    ) -> Option<&'a Endpoint> {
        let ip = match client_ip {
            Some(ip) => ip,
            None => return endpoints.first(),
        };

        let mut hasher = DefaultHasher::new();
        ip.hash(&mut hasher);
        let hash = hasher.finish();

        let idx = (hash as usize) % endpoints.len();
        endpoints.get(idx)
    }

    /// Increment load on an endpoint.
    pub fn inc_load(&self, endpoints: &mut [Endpoint], pod_id: &str) {
        for endpoint in endpoints.iter_mut() {
            if endpoint.pod_id == pod_id {
                endpoint.load += 1;
                return;
            }
        }
    }

    /// Decrement load on an endpoint.
    pub fn dec_load(&self, endpoints: &mut [Endpoint], pod_id: &str) {
        for endpoint in endpoints.iter_mut() {
            if endpoint.pod_id == pod_id {
                endpoint.load = endpoint.load.saturating_sub(1);
                return;
            }
        }
    }
}

impl Default for LoadBalancer {
    fn default() -> Self {
        Self::new(LoadBalancingStrategy::RoundRobin)
    }
}

use std::net::IpAddr;

#[cfg(test)]
mod tests {
    use super::*;

    fn make_endpoints(count: usize) -> Vec<Endpoint> {
        (0..count)
            .map(|i| Endpoint {
                pod_id: format!("pod-{}", i),
                ip: IpAddr::from([10, 0, 0, (i + 1) as u8]),
                port: 8080,
                healthy: true,
                load: 0,
            })
            .collect()
    }

    #[test]
    fn test_round_robin_distributes_evenly() {
        let lb = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
        let endpoints = make_endpoints(3);

        let selected: Vec<_> = (0..6)
            .map(|_| lb.select(&endpoints, None).unwrap().pod_id.clone())
            .collect();

        // Should cycle through all endpoints
        assert_eq!(
            selected,
            vec!["pod-0", "pod-1", "pod-2", "pod-0", "pod-1", "pod-2"]
        );
    }

    #[test]
    fn test_least_connections_picks_lowest_load() {
        let lb = LoadBalancer::new(LoadBalancingStrategy::LeastConnections);
        let mut endpoints = make_endpoints(3);
        endpoints[1].load = 10;
        endpoints[2].load = 5;

        let selected = lb.select(&endpoints, None).unwrap();
        assert_eq!(selected.pod_id, "pod-0"); // Has load 0
    }

    #[test]
    fn test_ip_hash_consistent() {
        let lb = LoadBalancer::new(LoadBalancingStrategy::IpHash);
        let endpoints = make_endpoints(3);
        let ip = IpAddr::from([192, 168, 1, 100]);

        let selected1 = lb.select(&endpoints, Some(ip)).unwrap();
        let selected2 = lb.select(&endpoints, Some(ip)).unwrap();

        // Same IP should always select same endpoint
        assert_eq!(selected1.pod_id, selected2.pod_id);
    }

    #[test]
    fn test_empty_endpoints_returns_none() {
        let lb = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);
        let endpoints: Vec<Endpoint> = vec![];

        let selected = lb.select(&endpoints, None);
        assert!(selected.is_none());
    }
}
