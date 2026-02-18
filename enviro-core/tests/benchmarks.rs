//! Performance benchmarks for Enviro Core.
//!
//! These tests measure timing for key operations and validate that
//! performance stays within acceptable bounds. Run with:
//!
//! ```sh
//! cargo test -p enviro-core --test benchmarks -- --ignored --nocapture
//! ```

use std::collections::HashMap;
use std::time::Instant;

use enviro_core::executor::{ExecutionContext, NetworkConfig, ResourceLimits};
use enviro_core::{
    BufferPool, ContextPool, NamespaceCache, NamespaceTemplate, OptimizedResourceLimits,
    ResourceLimitBatch, ResourceProfile,
};
use enviro_core::engine::resource_limits::ResourceKind;
use enviro_core::executor::NativeExecutor;
use enviro_core::Executor;

/// Helper to build a default [`ExecutionContext`] for benchmarks.
fn bench_context(id: &str) -> ExecutionContext {
    ExecutionContext {
        container_id: id.to_string(),
        env: HashMap::new(),
        workdir: "/tmp".to_string(),
        limits: ResourceLimits {
            cpu_cores: 1.0,
            memory_bytes: 256 * 1024 * 1024,
            pid_limit: 128,
        },
        network: NetworkConfig {
            isolated: true,
            ip_address: None,
            dns_servers: vec![],
        },
    }
}

// ---------------------------------------------------------------------------
// Container startup benchmark
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn bench_container_startup() {
    const ITERATIONS: usize = 1000;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let start = Instant::now();
    for i in 0..ITERATIONS {
        let ctx = bench_context(&format!("bench-{i}"));
        let mut executor = NativeExecutor::new();
        rt.block_on(async {
            executor.prepare(&ctx).await.unwrap();
        });
    }
    let elapsed = start.elapsed();
    let per_iter = elapsed / ITERATIONS as u32;

    eprintln!(
        "[bench_container_startup] {ITERATIONS} iterations in {elapsed:?} ({per_iter:?}/iter)"
    );
    assert!(
        per_iter.as_millis() < 10,
        "ExecutionContext + NativeExecutor::prepare should complete in < 10 ms, got {per_iter:?}"
    );
}

// ---------------------------------------------------------------------------
// Buffer pool benchmark
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn bench_buffer_pool_allocations() {
    const ITERATIONS: usize = 1000;
    let mut pool = BufferPool::new(4096);

    // Warm-up: seed the pool with a few buffers.
    for _ in 0..10 {
        let buf = pool.allocate();
        pool.release(buf);
    }

    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let mut buf = pool.allocate();
        buf.write(b"benchmark payload data for buffer pool test");
        pool.release(buf);
    }
    let elapsed = start.elapsed();
    let per_iter = elapsed / ITERATIONS as u32;

    let stats = pool.get_stats();
    eprintln!(
        "[bench_buffer_pool] {ITERATIONS} alloc+release cycles in {elapsed:?} ({per_iter:?}/iter)"
    );
    eprintln!(
        "  total_allocations={}, reuses={}, active={}",
        stats.total_allocations, stats.reuses, stats.active_count
    );
    assert!(
        stats.reuses >= ITERATIONS,
        "pool should reuse buffers after warm-up"
    );
    assert!(
        elapsed.as_millis() < 1000,
        "1000 buffer alloc+release cycles should finish in < 1 s, got {elapsed:?}"
    );
}

// ---------------------------------------------------------------------------
// Namespace cache benchmark
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn bench_namespace_cache() {
    const TEMPLATES: usize = 50;
    const LOOKUPS: usize = 10_000;
    let mut cache = NamespaceCache::new();

    // Populate the cache.
    for i in 0..TEMPLATES {
        cache.get_or_create(&format!("tpl-{i}"), || {
            NamespaceTemplate::new(format!("tpl-{i}"))
        });
    }

    // Benchmark cache hits.
    let start = Instant::now();
    for i in 0..LOOKUPS {
        let key = format!("tpl-{}", i % TEMPLATES);
        cache.get_or_create(&key, || panic!("should not be called on hit"));
    }
    let elapsed = start.elapsed();
    let per_iter = elapsed / LOOKUPS as u32;

    let stats = cache.cache_stats();
    eprintln!(
        "[bench_namespace_cache] {LOOKUPS} lookups in {elapsed:?} ({per_iter:?}/iter)"
    );
    eprintln!(
        "  hits={}, misses={}, cached={}",
        stats.hits, stats.misses, stats.cached_count
    );
    assert_eq!(stats.hits, LOOKUPS);
    assert!(
        elapsed.as_millis() < 1000,
        "10 000 cache lookups should finish in < 1 s, got {elapsed:?}"
    );

    // Benchmark cache misses (invalidate then re-create).
    let start_miss = Instant::now();
    for i in 0..TEMPLATES {
        let key = format!("tpl-{i}");
        cache.invalidate(&key);
        cache.get_or_create(&key, || NamespaceTemplate::new(&key));
    }
    let elapsed_miss = start_miss.elapsed();
    eprintln!(
        "[bench_namespace_cache_miss] {TEMPLATES} invalidate+recreate in {elapsed_miss:?}"
    );
}

// ---------------------------------------------------------------------------
// Resource limits batch benchmark
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn bench_resource_limits_batch() {
    const ITERATIONS: usize = 1000;

    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let mut batch = ResourceLimitBatch::new();
        batch.add_limit(ResourceKind::MemoryMax, 512 * 1024 * 1024);
        batch.add_limit(ResourceKind::MemoryHigh, 384 * 1024 * 1024);
        batch.add_limit(ResourceKind::CpuWeight, 100);
        batch.add_limit(ResourceKind::CpuMaxMicros, 100_000);
        batch.add_limit(ResourceKind::IoWeight, 100);
        batch.add_limit(ResourceKind::PidsMax, 512);
        batch.apply_batch().unwrap();
    }
    let elapsed = start.elapsed();
    let per_iter = elapsed / ITERATIONS as u32;

    eprintln!(
        "[bench_resource_limits_batch] {ITERATIONS} batch applies in {elapsed:?} ({per_iter:?}/iter)"
    );
    assert!(
        elapsed.as_millis() < 2000,
        "1000 batch applies should finish in < 2 s, got {elapsed:?}"
    );
}

#[test]
#[ignore]
fn bench_resource_profile_apply() {
    const ITERATIONS: usize = 1000;

    let profiles = [
        ResourceProfile::Minimal,
        ResourceProfile::Standard,
        ResourceProfile::Performance,
    ];

    for profile in &profiles {
        let start = Instant::now();
        for _ in 0..ITERATIONS {
            let limits = OptimizedResourceLimits::from_profile(profile.clone());
            limits.apply().unwrap();
        }
        let elapsed = start.elapsed();
        let per_iter = elapsed / ITERATIONS as u32;
        eprintln!(
            "[bench_resource_profile] {profile:?}: {ITERATIONS} applies in {elapsed:?} ({per_iter:?}/iter)"
        );
    }
}

// ---------------------------------------------------------------------------
// Context pool benchmark
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn bench_context_pool() {
    const ITERATIONS: usize = 1000;
    let mut pool = ContextPool::new(16);

    let start = Instant::now();
    for i in 0..ITERATIONS {
        let ctx = pool.acquire(format!("bench-{i}"));
        pool.release(ctx);
    }
    let elapsed = start.elapsed();
    let per_iter = elapsed / ITERATIONS as u32;

    let stats = pool.stats();
    eprintln!(
        "[bench_context_pool] {ITERATIONS} acquire+release cycles in {elapsed:?} ({per_iter:?}/iter)"
    );
    eprintln!(
        "  pool_size={}, recycled={}, peak={}",
        stats.pool_size, stats.recycled_count, stats.peak_usage
    );
    assert!(
        stats.recycled_count >= ITERATIONS as u64,
        "pool should recycle contexts"
    );
    assert!(
        elapsed.as_millis() < 1000,
        "1000 acquire+release cycles should finish in < 1 s, got {elapsed:?}"
    );
}
