const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    // Build static library for Rust FFI
    const lib = b.addStaticLibrary(.{
        .name = "enviro_zig",
        .root_source_file = .{ .path = "src/oom_tuner.zig" },
        .target = target,
        .optimize = optimize,
    });

    // Add allocator as additional source
    lib.addCSourceFile(.{
        .file = .{ .path = "src/allocator.zig" },
        .flags = &[_][]const u8{},
    });

    b.installArtifact(lib);

    // Tests
    const tests = b.addTest(.{
        .root_source_file = .{ .path = "src/oom_tuner.zig" },
        .target = target,
        .optimize = optimize,
    });

    const run_tests = b.addRunArtifact(tests);
    const test_step = b.step("test", "Run tests");
    test_step.dependOn(&run_tests.step);
}
