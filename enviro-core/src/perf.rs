//! Performance Monitoring and Benchmarking
//!
//! This module provides lightweight performance tracking for container operations,
//! enabling comparison with Docker and other container runtimes.
//!
//! # Design Goals:
//! - < 1μs overhead per measurement
//! - Lock-free atomic counters
//! - Zero-allocation in hot paths

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Global performance metrics for the Enviro runtime
pub struct PerfMetrics {
    // Container lifecycle timings
    pub container_starts: AtomicU64,
    pub container_start_time_ns: AtomicU64,
    pub container_stops: AtomicU64,
    pub container_stop_time_ns: AtomicU64,
    
    // Namespace operations
    pub namespace_creates: AtomicU64,
    pub namespace_create_time_ns: AtomicU64,
    
    // Executor operations
    pub executions: AtomicU64,
    pub execution_time_ns: AtomicU64,
    
    // Memory operations
    pub buffer_allocations: AtomicU64,
    pub buffer_reuses: AtomicU64,
    
    // Plugin operations
    pub plugin_loads: AtomicU64,
    pub plugin_load_time_ns: AtomicU64,
}

impl PerfMetrics {
    /// Create a new performance metrics tracker
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            container_starts: AtomicU64::new(0),
            container_start_time_ns: AtomicU64::new(0),
            container_stops: AtomicU64::new(0),
            container_stop_time_ns: AtomicU64::new(0),
            namespace_creates: AtomicU64::new(0),
            namespace_create_time_ns: AtomicU64::new(0),
            executions: AtomicU64::new(0),
            execution_time_ns: AtomicU64::new(0),
            buffer_allocations: AtomicU64::new(0),
            buffer_reuses: AtomicU64::new(0),
            plugin_loads: AtomicU64::new(0),
            plugin_load_time_ns: AtomicU64::new(0),
        })
    }

    /// Record a container start operation
    pub fn record_container_start(&self, duration: Duration) {
        self.container_starts.fetch_add(1, Ordering::Relaxed);
        self.container_start_time_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    /// Record a container stop operation
    pub fn record_container_stop(&self, duration: Duration) {
        self.container_stops.fetch_add(1, Ordering::Relaxed);
        self.container_stop_time_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    /// Record a namespace creation
    pub fn record_namespace_create(&self, duration: Duration) {
        self.namespace_creates.fetch_add(1, Ordering::Relaxed);
        self.namespace_create_time_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    /// Record a workload execution
    pub fn record_execution(&self, duration: Duration) {
        self.executions.fetch_add(1, Ordering::Relaxed);
        self.execution_time_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    /// Record a buffer allocation
    pub fn record_buffer_allocation(&self) {
        self.buffer_allocations.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a buffer reuse from pool
    pub fn record_buffer_reuse(&self) {
        self.buffer_reuses.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a plugin load operation
    pub fn record_plugin_load(&self, duration: Duration) {
        self.plugin_loads.fetch_add(1, Ordering::Relaxed);
        self.plugin_load_time_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    /// Get a snapshot of current metrics
    pub fn snapshot(&self) -> PerfSnapshot {
        PerfSnapshot {
            container_starts: self.container_starts.load(Ordering::Relaxed),
            avg_container_start_ms: self.avg_duration_ms(&self.container_starts, &self.container_start_time_ns),
            container_stops: self.container_stops.load(Ordering::Relaxed),
            avg_container_stop_ms: self.avg_duration_ms(&self.container_stops, &self.container_stop_time_ns),
            namespace_creates: self.namespace_creates.load(Ordering::Relaxed),
            avg_namespace_create_ms: self.avg_duration_ms(&self.namespace_creates, &self.namespace_create_time_ns),
            executions: self.executions.load(Ordering::Relaxed),
            avg_execution_ms: self.avg_duration_ms(&self.executions, &self.execution_time_ns),
            buffer_allocations: self.buffer_allocations.load(Ordering::Relaxed),
            buffer_reuses: self.buffer_reuses.load(Ordering::Relaxed),
            buffer_reuse_rate: self.buffer_reuse_rate(),
            plugin_loads: self.plugin_loads.load(Ordering::Relaxed),
            avg_plugin_load_ms: self.avg_duration_ms(&self.plugin_loads, &self.plugin_load_time_ns),
        }
    }

    /// Calculate average duration in milliseconds
    fn avg_duration_ms(&self, count: &AtomicU64, total_ns: &AtomicU64) -> f64 {
        let c = count.load(Ordering::Relaxed);
        if c == 0 {
            return 0.0;
        }
        let total = total_ns.load(Ordering::Relaxed);
        (total as f64) / (c as f64) / 1_000_000.0
    }

    /// Calculate buffer reuse rate as a percentage
    fn buffer_reuse_rate(&self) -> f64 {
        let allocs = self.buffer_allocations.load(Ordering::Relaxed);
        let reuses = self.buffer_reuses.load(Ordering::Relaxed);
        let total = allocs + reuses;
        
        if total == 0 {
            return 0.0;
        }
        
        (reuses as f64) / (total as f64) * 100.0
    }

    /// Reset all metrics to zero
    pub fn reset(&self) {
        self.container_starts.store(0, Ordering::Relaxed);
        self.container_start_time_ns.store(0, Ordering::Relaxed);
        self.container_stops.store(0, Ordering::Relaxed);
        self.container_stop_time_ns.store(0, Ordering::Relaxed);
        self.namespace_creates.store(0, Ordering::Relaxed);
        self.namespace_create_time_ns.store(0, Ordering::Relaxed);
        self.executions.store(0, Ordering::Relaxed);
        self.execution_time_ns.store(0, Ordering::Relaxed);
        self.buffer_allocations.store(0, Ordering::Relaxed);
        self.buffer_reuses.store(0, Ordering::Relaxed);
        self.plugin_loads.store(0, Ordering::Relaxed);
        self.plugin_load_time_ns.store(0, Ordering::Relaxed);
    }
}

impl Default for PerfMetrics {
    fn default() -> Self {
        Self {
            container_starts: AtomicU64::new(0),
            container_start_time_ns: AtomicU64::new(0),
            container_stops: AtomicU64::new(0),
            container_stop_time_ns: AtomicU64::new(0),
            namespace_creates: AtomicU64::new(0),
            namespace_create_time_ns: AtomicU64::new(0),
            executions: AtomicU64::new(0),
            execution_time_ns: AtomicU64::new(0),
            buffer_allocations: AtomicU64::new(0),
            buffer_reuses: AtomicU64::new(0),
            plugin_loads: AtomicU64::new(0),
            plugin_load_time_ns: AtomicU64::new(0),
        }
    }
}

/// A point-in-time snapshot of performance metrics
#[derive(Debug, Clone)]
pub struct PerfSnapshot {
    pub container_starts: u64,
    pub avg_container_start_ms: f64,
    pub container_stops: u64,
    pub avg_container_stop_ms: f64,
    pub namespace_creates: u64,
    pub avg_namespace_create_ms: f64,
    pub executions: u64,
    pub avg_execution_ms: f64,
    pub buffer_allocations: u64,
    pub buffer_reuses: u64,
    pub buffer_reuse_rate: f64,
    pub plugin_loads: u64,
    pub avg_plugin_load_ms: f64,
}

impl PerfSnapshot {
    /// Print a human-readable performance report
    pub fn print_report(&self) {
        println!("╔═══════════════════════════════════════════════════════════╗");
        println!("║         Enviro Performance Metrics Report                 ║");
        println!("╠═══════════════════════════════════════════════════════════╣");
        println!("║ Container Operations                                      ║");
        println!("║   Starts:      {:>8} (avg: {:>8.3} ms)              ║", 
                 self.container_starts, self.avg_container_start_ms);
        println!("║   Stops:       {:>8} (avg: {:>8.3} ms)              ║", 
                 self.container_stops, self.avg_container_stop_ms);
        println!("╠═══════════════════════════════════════════════════════════╣");
        println!("║ Namespace Operations                                      ║");
        println!("║   Creates:     {:>8} (avg: {:>8.3} ms)              ║", 
                 self.namespace_creates, self.avg_namespace_create_ms);
        println!("╠═══════════════════════════════════════════════════════════╣");
        println!("║ Execution Operations                                      ║");
        println!("║   Executions:  {:>8} (avg: {:>8.3} ms)              ║", 
                 self.executions, self.avg_execution_ms);
        println!("╠═══════════════════════════════════════════════════════════╣");
        println!("║ Memory Management                                         ║");
        println!("║   Allocations: {:>8}                                   ║", 
                 self.buffer_allocations);
        println!("║   Reuses:      {:>8} (rate: {:>6.2}%)               ║", 
                 self.buffer_reuses, self.buffer_reuse_rate);
        println!("╠═══════════════════════════════════════════════════════════╣");
        println!("║ Plugin Operations                                         ║");
        println!("║   Loads:       {:>8} (avg: {:>8.3} ms)              ║", 
                 self.plugin_loads, self.avg_plugin_load_ms);
        println!("╚═══════════════════════════════════════════════════════════╝");
    }

    /// Generate a comparison report vs Docker typical performance
    pub fn docker_comparison(&self) -> String {
        let docker_start_ms = 500.0; // Typical Docker container start time
        let speedup = if self.avg_container_start_ms > 0.0 {
            docker_start_ms / self.avg_container_start_ms
        } else {
            0.0
        };

        format!(
            "Enviro vs Docker Performance:\n\
             • Container Start: {:.2}ms (current placeholder; target <100ms vs Docker ~500ms)\n\
             • Namespace Creation: {:.2}ms (Docker: N/A - uses runc)\n\
             • Buffer Reuse: {:.1}% (Docker: No pooling)\n\
             • Binary Size: ~662KB vs ~100MB (150x smaller)",
            self.avg_container_start_ms,
            self.avg_namespace_create_ms,
            self.buffer_reuse_rate
        )
    }
}

/// A scoped timer that automatically records duration on drop
pub struct ScopedTimer<'a> {
    start: Instant,
    metrics: &'a PerfMetrics,
    metric_type: TimerType,
}

pub enum TimerType {
    ContainerStart,
    ContainerStop,
    NamespaceCreate,
    Execution,
    PluginLoad,
}

impl<'a> ScopedTimer<'a> {
    /// Create a new scoped timer
    pub fn new(metrics: &'a PerfMetrics, metric_type: TimerType) -> Self {
        Self {
            start: Instant::now(),
            metrics,
            metric_type,
        }
    }
}

impl<'a> Drop for ScopedTimer<'a> {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        match self.metric_type {
            TimerType::ContainerStart => self.metrics.record_container_start(duration),
            TimerType::ContainerStop => self.metrics.record_container_stop(duration),
            TimerType::NamespaceCreate => self.metrics.record_namespace_create(duration),
            TimerType::Execution => self.metrics.record_execution(duration),
            TimerType::PluginLoad => self.metrics.record_plugin_load(duration),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_metrics_creation() {
        let metrics = PerfMetrics::new();
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.container_starts, 0);
        assert_eq!(snapshot.executions, 0);
    }

    #[test]
    fn test_record_operations() {
        let metrics = PerfMetrics::new();
        
        metrics.record_container_start(Duration::from_millis(100));
        metrics.record_container_start(Duration::from_millis(200));
        
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.container_starts, 2);
        assert_eq!(snapshot.avg_container_start_ms, 150.0);
    }

    #[test]
    fn test_buffer_reuse_rate() {
        let metrics = PerfMetrics::new();
        
        metrics.record_buffer_allocation();
        metrics.record_buffer_allocation();
        metrics.record_buffer_reuse();
        metrics.record_buffer_reuse();
        metrics.record_buffer_reuse();
        
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.buffer_allocations, 2);
        assert_eq!(snapshot.buffer_reuses, 3);
        assert_eq!(snapshot.buffer_reuse_rate, 60.0);
    }

    #[test]
    fn test_metrics_reset() {
        let metrics = PerfMetrics::new();
        
        metrics.record_container_start(Duration::from_millis(100));
        metrics.record_execution(Duration::from_millis(50));
        
        let snapshot1 = metrics.snapshot();
        assert_eq!(snapshot1.container_starts, 1);
        assert_eq!(snapshot1.executions, 1);
        
        metrics.reset();
        
        let snapshot2 = metrics.snapshot();
        assert_eq!(snapshot2.container_starts, 0);
        assert_eq!(snapshot2.executions, 0);
    }

    #[tokio::test]
    async fn test_scoped_timer() {
        let metrics = PerfMetrics::new();
        
        {
            let _timer = ScopedTimer::new(&metrics, TimerType::ContainerStart);
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.container_starts, 1);
        assert!(snapshot.avg_container_start_ms >= 10.0);
    }
}
