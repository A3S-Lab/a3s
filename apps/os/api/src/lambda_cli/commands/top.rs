//! `a3s top` command - Show resource usage (kubectl top style).

use crate::commands::Command;
use crate::errors::Result;
use async_trait::async_trait;
use std::path::PathBuf;

/// Resource type to show metrics for.
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ResourceType {
    /// Show pod resource usage.
    Pods,
    /// Show node resource usage.
    Nodes,
}

/// Top command - show resource usage.
#[derive(clap::Parser, Debug)]
pub struct TopCommand {
    /// Resource type to show metrics for.
    #[arg(default_value = "pods")]
    resource: ResourceType,

    /// Namespace to filter by.
    #[arg(short, long, default_value = "default")]
    namespace: String,
}

impl TopCommand {
    fn boxes_dir() -> PathBuf {
        dirs::home_dir()
            .map(|h| h.join(".a3s").join("boxes"))
            .unwrap_or_else(|| PathBuf::from("~/.a3s/boxes"))
    }

    /// Format bytes into human-readable string.
    fn format_bytes(bytes: i64) -> String {
        const KB: i64 = 1024;
        const MB: i64 = KB * 1024;
        const GB: i64 = MB * 1024;

        if bytes >= GB {
            format!("{:.1}Gi", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.1}Mi", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.1}Ki", bytes as f64 / KB as f64)
        } else {
            format!("{}B", bytes)
        }
    }

    /// Show pod metrics.
    fn show_pods(&self) -> Result<()> {
        let boxes_dir = Self::boxes_dir();

        if !boxes_dir.exists() {
            println!("No pods found.");
            return Ok(());
        }

        println!(
            "{:<40} {:<12} {:<12} {:<10} {:<10}",
            "NAME", "CPU(cores)", "MEMORY(bytes)", "CPU%", "MEMORY%"
        );
        println!("{}", "-".repeat(90));

        let mut total_cpu = 0i64;
        let mut total_memory = 0i64;

        if let Ok(entries) = std::fs::read_dir(&boxes_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let pod_name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();

                    // Read metrics from info.json if available
                    let info_path = path.join("info.json");
                    let (cpu, memory) = if info_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&info_path) {
                            if let Ok(info) = serde_json::from_str::<serde_json::Value>(&content) {
                                let cpu = info
                                    .get("cpu_millicores")
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(0);
                                let memory = info
                                    .get("memory_bytes")
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(0);
                                (cpu, memory)
                            } else {
                                (0, 0)
                            }
                        } else {
                            (0, 0)
                        }
                    } else {
                        (0, 0)
                    };

                    total_cpu += cpu;
                    total_memory += memory;

                    let cpu_str = if cpu >= 1000 {
                        format!("{:.2}C", cpu as f64 / 1000.0)
                    } else {
                        format!("{}m", cpu)
                    };

                    println!(
                        "{:<40} {:<12} {:<12} {:<10} {:<10}",
                        pod_name,
                        cpu_str,
                        Self::format_bytes(memory),
                        "-",
                        "-"
                    );
                }
            }
        }

        println!("{}", "-".repeat(90));
        println!(
            "{:<40} {:<12} {:<12}",
            "Total",
            if total_cpu >= 1000 {
                format!("{:.2}C", total_cpu as f64 / 1000.0)
            } else {
                format!("{}m", total_cpu)
            },
            Self::format_bytes(total_memory)
        );

        Ok(())
    }

    /// Show node metrics.
    fn show_nodes(&self) -> Result<()> {
        println!("{}", "=".repeat(60));
        println!("Node: local");
        println!("{}", "=".repeat(60));

        // Calculate total resources from all pods
        let boxes_dir = Self::boxes_dir();
        let mut total_cpu = 0i64;
        let mut total_memory = 0i64;
        let mut running_pods = 0i32;

        if boxes_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&boxes_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        running_pods += 1;

                        let info_path = path.join("info.json");
                        if info_path.exists() {
                            if let Ok(content) = std::fs::read_to_string(&info_path) {
                                if let Ok(info) =
                                    serde_json::from_str::<serde_json::Value>(&content)
                                {
                                    total_cpu += info
                                        .get("cpu_millicores")
                                        .and_then(|v| v.as_i64())
                                        .unwrap_or(0);
                                    total_memory += info
                                        .get("memory_bytes")
                                        .and_then(|v| v.as_i64())
                                        .unwrap_or(0);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Assume total capacity (can be made configurable)
        let total_cpu_capacity = 4000i64; // 4 cores
        let total_memory_capacity = 16 * 1024 * 1024 * 1024i64; // 16Gi

        let cpu_percent = (total_cpu as f64 / total_cpu_capacity as f64 * 100.0).min(100.0);
        let memory_percent =
            (total_memory as f64 / total_memory_capacity as f64 * 100.0).min(100.0);

        println!();
        println!("Allocated Resources:");
        println!(
            "  CPU: {} ({}%)",
            if total_cpu >= 1000 {
                format!("{:.2}C", total_cpu as f64 / 1000.0)
            } else {
                format!("{}m", total_cpu)
            },
            format!("{:.1}", cpu_percent)
        );
        println!(
            "  Memory: {} ({}%)",
            Self::format_bytes(total_memory),
            format!("{:.1}", memory_percent)
        );
        println!("  Pods: {}", running_pods);
        println!();
        println!("Allocatable Resources:");
        println!("  CPU: 4C (4000m)");
        println!("  Memory: 16Gi");
        println!("  Pods: 110");

        Ok(())
    }
}

#[async_trait]
impl Command for TopCommand {
    async fn run(&self) -> Result<()> {
        match self.resource {
            ResourceType::Pods => self.show_pods(),
            ResourceType::Nodes => self.show_nodes(),
        }
    }
}
