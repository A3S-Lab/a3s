#!/bin/bash
# Test script for TSI inbound port forwarding in a3s-box
# Tests that external connections can reach services inside the VM

set -e

BOX_BIN="${BOX_BIN:-crates/box/src/target/debug/a3s-box}"
TEST_PORT="${TEST_PORT:-8080}"
GUEST_PORT="${GUEST_PORT:-80}"
CONTAINER_NAME="test-port-forward-$$"

# Set library path for libkrunfw
export DYLD_LIBRARY_PATH="/opt/homebrew/opt/a3s-box/lib:${DYLD_LIBRARY_PATH}"
export RUST_LOG="${RUST_LOG:-info}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

cleanup() {
    log_info "Cleaning up..."
    if $BOX_BIN ps | grep -q "$CONTAINER_NAME"; then
        log_info "Stopping container $CONTAINER_NAME"
        $BOX_BIN stop "$CONTAINER_NAME" 2>/dev/null || true
        $BOX_BIN rm "$CONTAINER_NAME" 2>/dev/null || true
    fi
}

# Trap cleanup on exit
trap cleanup EXIT

# Check if a3s-box binary exists
if [ ! -f "$BOX_BIN" ]; then
    log_error "a3s-box binary not found at $BOX_BIN"
    log_info "Build it with: cargo build -p a3s-box-cli"
    exit 1
fi

log_info "Testing TSI inbound port forwarding"
log_info "Host port: $TEST_PORT -> Guest port: $GUEST_PORT"

# Clean up any existing test container
cleanup

# Start nginx container with port mapping
log_info "Starting nginx container with port mapping ${TEST_PORT}:${GUEST_PORT}"
$BOX_BIN run -d \
    --name "$CONTAINER_NAME" \
    -p "${TEST_PORT}:${GUEST_PORT}" \
    nginx:alpine

# Wait for container to be ready
log_info "Waiting for container to start..."
sleep 3

# Check if container is running
if ! $BOX_BIN ps | grep -q "$CONTAINER_NAME"; then
    log_error "Container failed to start"
    exit 1
fi

log_info "Container started successfully"

# Wait for nginx to be ready inside the container
log_info "Waiting for nginx to be ready..."
sleep 2

# Test port forwarding from host
log_info "Testing connection from host to localhost:${TEST_PORT}"
MAX_RETRIES=10
RETRY_COUNT=0
SUCCESS=false

while [ $RETRY_COUNT -lt $MAX_RETRIES ]; do
    if curl -s -f -m 5 "http://localhost:${TEST_PORT}" > /dev/null 2>&1; then
        SUCCESS=true
        break
    fi
    RETRY_COUNT=$((RETRY_COUNT + 1))
    log_warn "Connection attempt $RETRY_COUNT/$MAX_RETRIES failed, retrying..."
    sleep 1
done

if [ "$SUCCESS" = true ]; then
    log_info "✓ Port forwarding test PASSED"
    log_info "Successfully connected to nginx through port ${TEST_PORT}"

    # Show response
    log_info "Response from nginx:"
    curl -s "http://localhost:${TEST_PORT}" | head -5

    exit 0
else
    log_error "✗ Port forwarding test FAILED"
    log_error "Could not connect to localhost:${TEST_PORT} after $MAX_RETRIES attempts"

    # Debug info
    log_info "Container status:"
    $BOX_BIN ps

    log_info "Container logs:"
    $BOX_BIN logs "$CONTAINER_NAME" 2>&1 | tail -20

    exit 1
fi
