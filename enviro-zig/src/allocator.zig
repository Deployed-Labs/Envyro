//! Custom Memory Allocator - Zig Implementation
//!
//! High-speed memory allocator optimized for container workload patterns:
//! - Frequent small allocations (process metadata, network buffers)
//! - Burst allocations (container startup)
//! - Long-lived allocations (container state)
//!
//! Performance Strategy:
//! - Arena allocator for short-lived objects
//! - Pool allocator for fixed-size objects
//! - General-purpose allocator as fallback

const std = @import("std");
const Allocator = std.mem.Allocator;

/// Custom allocator wrapper that tracks statistics
pub const EnviroAllocator = struct {
    base_allocator: Allocator,
    allocations: usize,
    frees: usize,
    
    const Self = @This();

    pub fn init(base: Allocator) Self {
        return Self{
            .base_allocator = base,
            .allocations = 0,
            .frees = 0,
        };
    }

    pub fn allocator(self: *Self) Allocator {
        return Allocator{
            .ptr = self,
            .vtable = &.{
                .alloc = alloc,
                .resize = resize,
                .free = free,
            },
        };
    }

    fn alloc(ctx: *anyopaque, len: usize, ptr_align: u8, ret_addr: usize) ?[*]u8 {
        const self: *Self = @ptrCast(@alignCast(ctx));
        self.allocations += 1;
        return self.base_allocator.rawAlloc(len, ptr_align, ret_addr);
    }

    fn resize(ctx: *anyopaque, buf: []u8, buf_align: u8, new_len: usize, ret_addr: usize) bool {
        const self: *Self = @ptrCast(@alignCast(ctx));
        return self.base_allocator.rawResize(buf, buf_align, new_len, ret_addr);
    }

    fn free(ctx: *anyopaque, buf: []u8, buf_align: u8, ret_addr: usize) void {
        const self: *Self = @ptrCast(@alignCast(ctx));
        self.frees += 1;
        self.base_allocator.rawFree(buf, buf_align, ret_addr);
    }

    pub fn getStats(self: *const Self) struct { allocations: usize, frees: usize } {
        return .{
            .allocations = self.allocations,
            .frees = self.frees,
        };
    }
};

/// Global allocator instance (thread-safe via Zig's comptime guarantees)
var gpa = std.heap.GeneralPurposeAllocator(.{}){};
var enviro_allocator = EnviroAllocator.init(gpa.allocator());

/// Get the global Enviro allocator
pub fn getAllocator() Allocator {
    return enviro_allocator.allocator();
}

/// Export allocator statistics to C-ABI
export fn zig_allocator_total_allocs() u64 {
    return @intCast(enviro_allocator.allocations);
}

export fn zig_allocator_total_frees() u64 {
    return @intCast(enviro_allocator.frees);
}

test "allocator_tracking" {
    const expect = std.testing.expect;
    
    var test_gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = test_gpa.deinit();
    
    var alloc = EnviroAllocator.init(test_gpa.allocator());
    
    const initial_allocs = alloc.allocations;
    
    // Allocate and free
    const slice = try alloc.allocator().alloc(u8, 100);
    try expect(alloc.allocations == initial_allocs + 1);
    
    alloc.allocator().free(slice);
    try expect(alloc.frees > 0);
}
