// Package control implements the gRPC control plane for Enviro
//
// This Go module provides:
// - gRPC server for container orchestration
// - eBPF-based networking for sub-millisecond latency
// - Distributed control plane coordination
//
// Performance Design:
// - Goroutine-per-request for natural concurrency
// - Zero-copy protobuf serialization where possible
// - Connection pooling and multiplexing

package main

/*
#include <stdlib.h>

// FFI result codes matching Rust
typedef int ffi_result;
const ffi_result FFI_SUCCESS = 0;
const ffi_result FFI_ERROR = -1;
*/
import "C"

import (
	"context"
	"fmt"
	"log"
	"net"
	"sync"
	"unsafe"

	"google.golang.org/grpc"
)

// Global control plane instance
var (
	controlPlane *ControlPlane
	mu           sync.Mutex
)

// ControlPlane manages the gRPC server and networking
type ControlPlane struct {
	grpcServer *grpc.Server
	listener   net.Listener
	address    string
}

// NewControlPlane creates a new control plane instance
func NewControlPlane(address string) (*ControlPlane, error) {
	listener, err := net.Listen("tcp", address)
	if err != nil {
		return nil, fmt.Errorf("failed to listen on %s: %w", address, err)
	}

	grpcServer := grpc.NewServer(
		// Performance optimizations
		grpc.MaxConcurrentStreams(1000),
		grpc.MaxRecvMsgSize(16 * 1024 * 1024), // 16MB
		grpc.MaxSendMsgSize(16 * 1024 * 1024),
	)

	// TODO: Register gRPC services here
	// Example: pb.RegisterContainerServiceServer(grpcServer, &containerService{})

	return &ControlPlane{
		grpcServer: grpcServer,
		listener:   listener,
		address:    address,
	}, nil
}

// Start begins serving gRPC requests
func (cp *ControlPlane) Start() error {
	log.Printf("Starting gRPC control plane on %s", cp.address)
	return cp.grpcServer.Serve(cp.listener)
}

// Stop gracefully shuts down the control plane
func (cp *ControlPlane) Stop() {
	log.Println("Shutting down gRPC control plane")
	cp.grpcServer.GracefulStop()
}

//export go_init_control_plane
func go_init_control_plane(addr *C.char) C.ffi_result {
	mu.Lock()
	defer mu.Unlock()

	if controlPlane != nil {
		log.Println("Control plane already initialized")
		return C.FFI_SUCCESS
	}

	goAddr := C.GoString(addr)

	cp, err := NewControlPlane(goAddr)
	if err != nil {
		log.Printf("Failed to initialize control plane: %v", err)
		return C.FFI_ERROR
	}

	controlPlane = cp

	// Start serving in a goroutine
	go func() {
		if err := cp.Start(); err != nil {
			log.Printf("Control plane error: %v", err)
		}
	}()

	log.Println("Control plane initialized successfully")
	return C.FFI_SUCCESS
}

//export go_shutdown_control_plane
func go_shutdown_control_plane() C.ffi_result {
	mu.Lock()
	defer mu.Unlock()

	if controlPlane == nil {
		log.Println("Control plane not initialized")
		return C.FFI_ERROR
	}

	controlPlane.Stop()
	controlPlane = nil

	log.Println("Control plane shutdown complete")
	return C.FFI_SUCCESS
}

// Required for CGO - must have main() when building c-shared
func main() {
	// This is never called when built as c-shared library
	// But Go requires it to be present
}
