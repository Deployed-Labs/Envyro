//! Zero-Copy Buffer Management for Container I/O
//!
//! This module provides a pool-based buffer management system designed to
//! minimise allocation overhead during high-throughput container I/O operations
//! (layer unpacking, overlay diffs, checkpoint/restore streams).
//!
//! # Performance-First Design:
//! - Pre-allocated buffers avoid per-operation `malloc`/`free` overhead
//! - `BufferPool` recycles released buffers so hot paths never touch the allocator
//! - Statistics tracking enables runtime tuning of pool sizes

use std::collections::VecDeque;
use tracing::{debug, info};

/// Default capacity in bytes for a newly allocated buffer.
pub const DEFAULT_BUFFER_CAPACITY: usize = 4096;

/// A managed memory buffer for container I/O operations.
///
/// `ZeroCopyBuffer` wraps a contiguous byte region that can be written to
/// and read from without intermediate copies.  Buffers are intended to be
/// obtained from a [`BufferPool`] and returned after use so the underlying
/// allocation can be reused.
///
/// # Performance Pattern: Buffer Reuse
/// Rather than allocating a fresh `Vec<u8>` for every I/O operation, create
/// a `BufferPool`, call [`BufferPool::allocate`] to obtain a buffer, and
/// call [`BufferPool::release`] when done.  This keeps the hot path
/// allocation-free after the pool is warmed up.
#[derive(Debug)]
pub struct ZeroCopyBuffer {
    /// Underlying storage.
    data: Vec<u8>,
    /// Logical length of valid data (may be less than `data.capacity()`).
    len: usize,
    /// The capacity this buffer was created with (used for stats/matching).
    capacity: usize,
}

impl ZeroCopyBuffer {
    /// Create a new buffer with the given capacity.
    ///
    /// The buffer starts empty (`len == 0`) but has room for `capacity` bytes
    /// without further allocation.
    pub fn new(capacity: usize) -> Self {
        debug!(capacity, "Allocating ZeroCopyBuffer");
        Self {
            data: Vec::with_capacity(capacity),
            len: 0,
            capacity,
        }
    }

    /// Write `src` into the buffer, replacing any previous contents.
    ///
    /// The buffer grows automatically if `src` exceeds the current capacity,
    /// but for best performance callers should pre-size via the pool.
    pub fn write(&mut self, src: &[u8]) {
        self.data.clear();
        self.data.extend_from_slice(src);
        self.len = src.len();
    }

    /// Returns a slice over the valid data in the buffer.
    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.len]
    }

    /// Returns the number of valid bytes currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when the buffer contains no valid data.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the total allocated capacity in bytes.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Reset the buffer for reuse, keeping the allocation.
    fn reset(&mut self) {
        self.data.clear();
        self.len = 0;
    }
}

/// Runtime statistics for a [`BufferPool`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferPoolStats {
    /// Total number of buffers allocated since pool creation.
    pub total_allocations: usize,
    /// Number of times an existing buffer was reused from the pool.
    pub reuses: usize,
    /// Number of buffers currently checked out (not yet released).
    pub active_count: usize,
}

/// A pool of reusable [`ZeroCopyBuffer`]s.
///
/// `BufferPool` maintains a free-list of previously allocated buffers.  When
/// a caller requests a buffer via [`allocate`](Self::allocate) the pool first
/// checks the free-list; only when no suitable buffer is available does it
/// fall back to a fresh allocation.
///
/// # Performance Pattern: Pool Warming
/// Call `allocate` / `release` in a tight loop during initialisation to
/// pre-populate the pool.  Subsequent I/O operations then run without
/// touching the system allocator.
pub struct BufferPool {
    /// Default capacity for newly created buffers.
    default_capacity: usize,
    /// Free-list of returned buffers ready for reuse.
    free_list: VecDeque<ZeroCopyBuffer>,
    /// Cumulative statistics.
    total_allocations: usize,
    reuses: usize,
    active_count: usize,
}

impl BufferPool {
    /// Create a new, empty buffer pool.
    ///
    /// Buffers allocated from this pool default to `capacity` bytes.
    pub fn new(capacity: usize) -> Self {
        info!(capacity, "Creating BufferPool");
        Self {
            default_capacity: capacity,
            free_list: VecDeque::new(),
            total_allocations: 0,
            reuses: 0,
            active_count: 0,
        }
    }

    /// Obtain a buffer from the pool.
    ///
    /// If the free-list contains a buffer it is returned immediately (reuse).
    /// Otherwise a new buffer is allocated with the pool's default capacity.
    pub fn allocate(&mut self) -> ZeroCopyBuffer {
        let buf = if let Some(mut buf) = self.free_list.pop_front() {
            buf.reset();
            self.reuses += 1;
            debug!("Reusing buffer from pool");
            buf
        } else {
            self.total_allocations += 1;
            debug!("Allocating new buffer");
            ZeroCopyBuffer::new(self.default_capacity)
        };
        self.active_count += 1;
        buf
    }

    /// Return a buffer to the pool for future reuse.
    ///
    /// The buffer's contents are cleared but the underlying allocation is
    /// retained so the next [`allocate`](Self::allocate) call is free.
    pub fn release(&mut self, buf: ZeroCopyBuffer) {
        self.active_count = self.active_count.saturating_sub(1);
        self.free_list.push_back(buf);
        debug!(free = self.free_list.len(), "Buffer released to pool");
    }

    /// Snapshot the pool's runtime statistics.
    pub fn get_stats(&self) -> BufferPoolStats {
        BufferPoolStats {
            total_allocations: self.total_allocations,
            reuses: self.reuses,
            active_count: self.active_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_new() {
        let buf = ZeroCopyBuffer::new(1024);
        assert_eq!(buf.len(), 0);
        assert!(buf.is_empty());
        assert_eq!(buf.capacity(), 1024);
    }

    #[test]
    fn test_buffer_write_and_read() {
        let mut buf = ZeroCopyBuffer::new(64);
        buf.write(b"hello world");
        assert_eq!(buf.len(), 11);
        assert!(!buf.is_empty());
        assert_eq!(buf.as_slice(), b"hello world");
    }

    #[test]
    fn test_buffer_overwrite() {
        let mut buf = ZeroCopyBuffer::new(64);
        buf.write(b"first");
        buf.write(b"second");
        assert_eq!(buf.as_slice(), b"second");
        assert_eq!(buf.len(), 6);
    }

    #[test]
    fn test_buffer_reset() {
        let mut buf = ZeroCopyBuffer::new(64);
        buf.write(b"data");
        buf.reset();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn test_pool_allocate_fresh() {
        let mut pool = BufferPool::new(DEFAULT_BUFFER_CAPACITY);
        let buf = pool.allocate();
        assert_eq!(buf.capacity(), DEFAULT_BUFFER_CAPACITY);

        let stats = pool.get_stats();
        assert_eq!(stats.total_allocations, 1);
        assert_eq!(stats.reuses, 0);
        assert_eq!(stats.active_count, 1);
    }

    #[test]
    fn test_pool_reuse() {
        let mut pool = BufferPool::new(DEFAULT_BUFFER_CAPACITY);

        let buf = pool.allocate();
        pool.release(buf);

        let _buf2 = pool.allocate();

        let stats = pool.get_stats();
        assert_eq!(stats.total_allocations, 1);
        assert_eq!(stats.reuses, 1);
        assert_eq!(stats.active_count, 1);
    }

    #[test]
    fn test_pool_multiple_allocations() {
        let mut pool = BufferPool::new(256);

        let b1 = pool.allocate();
        let b2 = pool.allocate();
        let b3 = pool.allocate();

        assert_eq!(pool.get_stats().active_count, 3);
        assert_eq!(pool.get_stats().total_allocations, 3);

        pool.release(b1);
        pool.release(b2);
        pool.release(b3);

        assert_eq!(pool.get_stats().active_count, 0);

        // All three should be reused now
        let _r1 = pool.allocate();
        let _r2 = pool.allocate();
        let _r3 = pool.allocate();

        let stats = pool.get_stats();
        assert_eq!(stats.total_allocations, 3);
        assert_eq!(stats.reuses, 3);
        assert_eq!(stats.active_count, 3);
    }

    #[test]
    fn test_pool_released_buffer_is_empty() {
        let mut pool = BufferPool::new(128);
        let mut buf = pool.allocate();
        buf.write(b"leftover data");
        pool.release(buf);

        let reused = pool.allocate();
        assert!(reused.is_empty(), "reused buffer should be cleared");
    }
}
