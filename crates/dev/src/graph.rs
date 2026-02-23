use std::collections::{HashMap, VecDeque};

use crate::config::DevConfig;
use crate::error::{DevError, Result};

pub struct DependencyGraph {
    /// Services in topological start order
    order: Vec<String>,
}

impl DependencyGraph {
    pub fn from_config(cfg: &DevConfig) -> Result<Self> {
        let names: Vec<&str> = cfg.service.keys().map(|s| s.as_str()).collect();

        // Build adjacency: name -> list of names that depend on it (reverse edges for Kahn's)
        let mut in_degree: HashMap<&str, usize> = names.iter().map(|n| (*n, 0)).collect();
        let mut dependents: HashMap<&str, Vec<&str>> = names.iter().map(|n| (*n, vec![])).collect();

        for (name, svc) in &cfg.service {
            for dep in &svc.depends_on {
                *in_degree.entry(name.as_str()).or_insert(0) += 1;
                dependents.entry(dep.as_str()).or_default().push(name.as_str());
            }
        }

        // Kahn's algorithm — preserve declaration order as tiebreaker
        let mut queue: VecDeque<&str> = names
            .iter()
            .filter(|n| in_degree[*n] == 0)
            .copied()
            .collect();

        let mut order = Vec::with_capacity(names.len());
        while let Some(node) = queue.pop_front() {
            order.push(node.to_string());
            for &dep in &dependents[node] {
                let deg = in_degree.get_mut(dep).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.push_back(dep);
                }
            }
        }

        if order.len() < names.len() {
            // Find nodes still in cycle
            let cycled: Vec<&str> = names
                .iter()
                .filter(|n| !order.iter().any(|o| o == *n))
                .copied()
                .collect();
            return Err(DevError::Cycle(cycled.join(", ")));
        }

        Ok(Self { order })
    }

    pub fn start_order(&self) -> &[String] {
        &self.order
    }

    pub fn stop_order(&self) -> impl Iterator<Item = &str> {
        self.order.iter().rev().map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DevConfig, ServiceDef};
    use indexmap::IndexMap;

    fn make_config(services: Vec<(&str, Vec<&str>)>) -> DevConfig {
        let mut map = IndexMap::new();
        for (i, (name, deps)) in services.into_iter().enumerate() {
            map.insert(
                name.to_string(),
                ServiceDef {
                    cmd: "echo".into(),
                    dir: None,
                    port: 8000 + i as u16,
                    subdomain: None,
                    env: Default::default(),
                    depends_on: deps.iter().map(|s| s.to_string()).collect(),
                    watch: None,
                    health: None,
                },
            );
        }
        DevConfig {
            dev: Default::default(),
            brew: Default::default(),
            service: map,
        }
    }

    #[test]
    fn test_simple_order() {
        let cfg = make_config(vec![("b", vec!["a"]), ("a", vec![])]);
        let g = DependencyGraph::from_config(&cfg).unwrap();
        let order = g.start_order();
        assert!(order.iter().position(|s| s == "a") < order.iter().position(|s| s == "b"));
    }

    #[test]
    fn test_cycle_detected() {
        let cfg = make_config(vec![("a", vec!["b"]), ("b", vec!["a"])]);
        assert!(DependencyGraph::from_config(&cfg).is_err());
    }
}
