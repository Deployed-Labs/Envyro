//! Container Monitor - Track and Observe Running Environments
//!
//! Provides real-time status tracking for running containers, including
//! state management, resource usage snapshots, and event logging.
//!
//! # Design:
//! - Lock-free reads via `Arc<RwLock<…>>` for concurrent monitoring
//! - Lightweight event log for debugging
//! - Human-readable status output for CLI

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Possible states of a container
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerState {
    /// Being created
    Creating,
    /// Running normally
    Running,
    /// Paused / frozen
    Paused,
    /// Stopped gracefully
    Stopped,
    /// Exited with an error
    Failed,
}

impl std::fmt::Display for ContainerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainerState::Creating => write!(f, "Creating"),
            ContainerState::Running => write!(f, "Running"),
            ContainerState::Paused => write!(f, "Paused"),
            ContainerState::Stopped => write!(f, "Stopped"),
            ContainerState::Failed => write!(f, "Failed"),
        }
    }
}

/// Status information for a single container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerStatus {
    /// Container ID
    pub id: String,
    /// Environment name (from Envirofile)
    pub name: String,
    /// Current state
    pub state: ContainerState,
    /// When the container was created
    pub created_at: DateTime<Utc>,
    /// When the container started running (if applicable)
    pub started_at: Option<DateTime<Utc>>,
    /// When the container stopped (if applicable)
    pub stopped_at: Option<DateTime<Utc>>,
    /// Image / base used
    pub image: String,
    /// Command being run
    pub command: String,
    /// Exposed ports
    pub ports: Vec<u16>,
    /// Resource usage snapshot
    pub resources: ResourceUsage,
    /// Exit code (if stopped/failed)
    pub exit_code: Option<i32>,
}

/// Snapshot of resource usage
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// CPU usage percentage (0-100)
    pub cpu_percent: f64,
    /// Memory used in bytes
    pub memory_bytes: u64,
    /// Memory limit in bytes
    pub memory_limit: u64,
    /// Number of running processes
    pub pids: u32,
    /// Network bytes received
    pub net_rx_bytes: u64,
    /// Network bytes transmitted
    pub net_tx_bytes: u64,
}

/// Event in a container's lifecycle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerEvent {
    /// Timestamp of the event
    pub timestamp: DateTime<Utc>,
    /// Container ID
    pub container_id: String,
    /// Event type
    pub event_type: EventType,
    /// Human-readable message
    pub message: String,
}

/// Types of container lifecycle events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    Created,
    Started,
    Stopped,
    Failed,
    HealthCheck,
    OomKilled,
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventType::Created => write!(f, "Created"),
            EventType::Started => write!(f, "Started"),
            EventType::Stopped => write!(f, "Stopped"),
            EventType::Failed => write!(f, "Failed"),
            EventType::HealthCheck => write!(f, "HealthCheck"),
            EventType::OomKilled => write!(f, "OomKilled"),
        }
    }
}

/// Container monitor that tracks all running containers
pub struct Monitor {
    containers: Arc<RwLock<HashMap<String, ContainerStatus>>>,
    events: Arc<RwLock<Vec<ContainerEvent>>>,
    max_events: usize,
}

impl Monitor {
    /// Create a new monitor
    pub fn new() -> Self {
        Self {
            containers: Arc::new(RwLock::new(HashMap::new())),
            events: Arc::new(RwLock::new(Vec::new())),
            max_events: 1000,
        }
    }

    /// Register a new container
    pub async fn register(
        &self,
        id: String,
        name: String,
        image: String,
        command: String,
        ports: Vec<u16>,
        memory_limit: u64,
    ) {
        let now = Utc::now();
        let status = ContainerStatus {
            id: id.clone(),
            name,
            state: ContainerState::Creating,
            created_at: now,
            started_at: None,
            stopped_at: None,
            image,
            command,
            ports,
            resources: ResourceUsage {
                memory_limit,
                ..Default::default()
            },
            exit_code: None,
        };

        self.containers.write().await.insert(id.clone(), status);
        self.add_event(id, EventType::Created, "Container created".to_string())
            .await;
    }

    /// Mark a container as running
    pub async fn mark_running(&self, id: &str) {
        if let Some(status) = self.containers.write().await.get_mut(id) {
            status.state = ContainerState::Running;
            status.started_at = Some(Utc::now());
        }
        self.add_event(
            id.to_string(),
            EventType::Started,
            "Container started".to_string(),
        )
        .await;
    }

    /// Mark a container as stopped
    pub async fn mark_stopped(&self, id: &str, exit_code: i32) {
        if let Some(status) = self.containers.write().await.get_mut(id) {
            status.state = if exit_code == 0 {
                ContainerState::Stopped
            } else {
                ContainerState::Failed
            };
            status.stopped_at = Some(Utc::now());
            status.exit_code = Some(exit_code);
        }

        let event_type = if exit_code == 0 {
            EventType::Stopped
        } else {
            EventType::Failed
        };
        self.add_event(
            id.to_string(),
            event_type,
            format!("Container exited with code {}", exit_code),
        )
        .await;
    }

    /// Update resource usage for a container
    pub async fn update_resources(&self, id: &str, resources: ResourceUsage) {
        if let Some(status) = self.containers.write().await.get_mut(id) {
            status.resources = resources;
        }
    }

    /// Get status of a specific container
    pub async fn get_status(&self, id: &str) -> Option<ContainerStatus> {
        self.containers.read().await.get(id).cloned()
    }

    /// List all containers (optionally filtered by state)
    pub async fn list(&self, state_filter: Option<ContainerState>) -> Vec<ContainerStatus> {
        let containers = self.containers.read().await;
        containers
            .values()
            .filter(|c| state_filter.is_none_or(|s| c.state == s))
            .cloned()
            .collect()
    }

    /// Get recent events (optionally filtered by container ID)
    pub async fn events(&self, container_id: Option<&str>) -> Vec<ContainerEvent> {
        let events = self.events.read().await;
        match container_id {
            Some(id) => events.iter().filter(|e| e.container_id == id).cloned().collect(),
            None => events.clone(),
        }
    }

    /// Remove a stopped container from tracking
    pub async fn remove(&self, id: &str) -> bool {
        self.containers.write().await.remove(id).is_some()
    }

    /// Get count of containers by state
    pub async fn summary(&self) -> MonitorSummary {
        let containers = self.containers.read().await;
        let mut summary = MonitorSummary::default();

        for status in containers.values() {
            summary.total += 1;
            match status.state {
                ContainerState::Creating => summary.creating += 1,
                ContainerState::Running => summary.running += 1,
                ContainerState::Paused => summary.paused += 1,
                ContainerState::Stopped => summary.stopped += 1,
                ContainerState::Failed => summary.failed += 1,
            }
        }

        summary
    }

    // --- Private helpers ---

    async fn add_event(&self, container_id: String, event_type: EventType, message: String) {
        let event = ContainerEvent {
            timestamp: Utc::now(),
            container_id,
            event_type,
            message,
        };

        let mut events = self.events.write().await;
        events.push(event);

        // Trim old events if exceeding max
        if events.len() > self.max_events {
            let drain_count = events.len() - self.max_events;
            events.drain(..drain_count);
        }
    }
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Monitor {
    fn clone(&self) -> Self {
        Self {
            containers: self.containers.clone(),
            events: self.events.clone(),
            max_events: self.max_events,
        }
    }
}

/// Summary of container states
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MonitorSummary {
    pub total: usize,
    pub creating: usize,
    pub running: usize,
    pub paused: usize,
    pub stopped: usize,
    pub failed: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_monitor_register() {
        let monitor = Monitor::new();
        monitor
            .register(
                "c1".into(),
                "test".into(),
                "ubuntu".into(),
                "/bin/sh".into(),
                vec![8080],
                512 * 1024 * 1024,
            )
            .await;

        let status = monitor.get_status("c1").await.unwrap();
        assert_eq!(status.state, ContainerState::Creating);
        assert_eq!(status.name, "test");
        assert_eq!(status.ports, vec![8080]);
    }

    #[tokio::test]
    async fn test_monitor_lifecycle() {
        let monitor = Monitor::new();
        monitor
            .register(
                "c1".into(),
                "test".into(),
                "ubuntu".into(),
                "/bin/sh".into(),
                vec![],
                0,
            )
            .await;

        monitor.mark_running("c1").await;
        let status = monitor.get_status("c1").await.unwrap();
        assert_eq!(status.state, ContainerState::Running);
        assert!(status.started_at.is_some());

        monitor.mark_stopped("c1", 0).await;
        let status = monitor.get_status("c1").await.unwrap();
        assert_eq!(status.state, ContainerState::Stopped);
        assert_eq!(status.exit_code, Some(0));
    }

    #[tokio::test]
    async fn test_monitor_failed() {
        let monitor = Monitor::new();
        monitor
            .register(
                "c1".into(),
                "test".into(),
                "ubuntu".into(),
                "/bin/sh".into(),
                vec![],
                0,
            )
            .await;
        monitor.mark_running("c1").await;
        monitor.mark_stopped("c1", 1).await;

        let status = monitor.get_status("c1").await.unwrap();
        assert_eq!(status.state, ContainerState::Failed);
        assert_eq!(status.exit_code, Some(1));
    }

    #[tokio::test]
    async fn test_monitor_list_filter() {
        let monitor = Monitor::new();
        monitor
            .register("c1".into(), "a".into(), "img".into(), "cmd".into(), vec![], 0)
            .await;
        monitor
            .register("c2".into(), "b".into(), "img".into(), "cmd".into(), vec![], 0)
            .await;
        monitor.mark_running("c1").await;

        let all = monitor.list(None).await;
        assert_eq!(all.len(), 2);

        let running = monitor.list(Some(ContainerState::Running)).await;
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].id, "c1");
    }

    #[tokio::test]
    async fn test_monitor_events() {
        let monitor = Monitor::new();
        monitor
            .register("c1".into(), "a".into(), "img".into(), "cmd".into(), vec![], 0)
            .await;
        monitor.mark_running("c1").await;

        let events = monitor.events(Some("c1")).await;
        assert_eq!(events.len(), 2); // Created + Started
    }

    #[tokio::test]
    async fn test_monitor_summary() {
        let monitor = Monitor::new();
        monitor
            .register("c1".into(), "a".into(), "img".into(), "cmd".into(), vec![], 0)
            .await;
        monitor
            .register("c2".into(), "b".into(), "img".into(), "cmd".into(), vec![], 0)
            .await;
        monitor
            .register("c3".into(), "c".into(), "img".into(), "cmd".into(), vec![], 0)
            .await;
        monitor.mark_running("c1").await;
        monitor.mark_running("c2").await;
        monitor.mark_stopped("c2", 1).await;

        let summary = monitor.summary().await;
        assert_eq!(summary.total, 3);
        assert_eq!(summary.running, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.creating, 1);
    }

    #[tokio::test]
    async fn test_monitor_remove() {
        let monitor = Monitor::new();
        monitor
            .register("c1".into(), "a".into(), "img".into(), "cmd".into(), vec![], 0)
            .await;

        assert!(monitor.remove("c1").await);
        assert!(!monitor.remove("c1").await);
        assert!(monitor.get_status("c1").await.is_none());
    }

    #[tokio::test]
    async fn test_update_resources() {
        let monitor = Monitor::new();
        monitor
            .register("c1".into(), "a".into(), "img".into(), "cmd".into(), vec![], 1024)
            .await;

        monitor
            .update_resources(
                "c1",
                ResourceUsage {
                    cpu_percent: 25.5,
                    memory_bytes: 256,
                    memory_limit: 1024,
                    pids: 5,
                    net_rx_bytes: 100,
                    net_tx_bytes: 200,
                },
            )
            .await;

        let status = monitor.get_status("c1").await.unwrap();
        assert_eq!(status.resources.cpu_percent, 25.5);
        assert_eq!(status.resources.pids, 5);
    }

    #[test]
    fn test_container_state_display() {
        assert_eq!(ContainerState::Running.to_string(), "Running");
        assert_eq!(ContainerState::Failed.to_string(), "Failed");
        assert_eq!(ContainerState::Creating.to_string(), "Creating");
    }

    #[test]
    fn test_event_type_display() {
        assert_eq!(EventType::Created.to_string(), "Created");
        assert_eq!(EventType::OomKilled.to_string(), "OomKilled");
    }
}
