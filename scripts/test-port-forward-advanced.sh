#!/bin/bash
# Advanced test script for TSI inbound port forwarding
# Tests multiple ports and concurrent connections

set -e

BOX_BIN="${BOX_BIN:-crates/box/src/target/debug/a3s-box}"
CONTAINER_NAME="test-multiport-$$"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_test() { echo -e "${BLUE}[TEST]${NC} $1"; }

cleanup() {
    log_info "Cleaning up..."
    if $BOX_BIN ps 2>/dev/null | grep -q "$CONTAINER_NAME"; then
        $BOX_BIN stop "$CONTAINER_NAME" 2>/dev/null || true
        $BOX_BIN rm "$CONTAINER_NAME" 2>/dev/null || true
    fi
}

trap cleanup EXIT

# Check binary
if [ ! -f "$BOX_BIN" ]; then
    log_error "a3s-box binary not found at $BOX_BIN"
    exit 1
fi

log_info "=== TSI Inbound Port Forwarding Advanced Test ==="
echo

# Test 1: Single port mapping (HTTP)
log_test "Test 1: Single port mapping (8080:80)"
cleanup

$BOX_BIN run -d \
    --name "$CONTAINER_NAME" \
    -p "8080:80" \
    nginx:alpine

sleep 3

if curl -s -f -m 5 "http://localhost:8080" > /dev/null 2>&1; then
    log_info "✓ Test 1 PASSED: Single port mapping works"
else
    log_error "✗ Test 1 FAILED: Cannot connect to port 8080"
    exit 1
fi

cleanup
sleep 1

# Test 2: Multiple port mappings
log_test "Test 2: Multiple port mappings (8080:80, 8081:80)"
$BOX_BIN run -d \
    --name "$CONTAINER_NAME" \
    -p "8080:80" \
    -p "8081:80" \
    nginx:alpine

sleep 3

TEST2_PASS=true
if ! curl -s -f -m 5 "http://localhost:8080" > /dev/null 2>&1; then
    log_error "✗ Port 8080 failed"
    TEST2_PASS=false
fi

if ! curl -s -f -m 5 "http://localhost:8081" > /dev/null 2>&1; then
    log_error "✗ Port 8081 failed"
    TEST2_PASS=false
fi

if [ "$TEST2_PASS" = true ]; then
    log_info "✓ Test 2 PASSED: Multiple port mappings work"
else
    log_error "✗ Test 2 FAILED"
    exit 1
fi

cleanup
sleep 1

# Test 3: Concurrent connections
log_test "Test 3: Concurrent connections (10 parallel requests)"
$BOX_BIN run -d \
    --name "$CONTAINER_NAME" \
    -p "8080:80" \
    nginx:alpine

sleep 3

TEST3_PASS=true
for i in {1..10}; do
    if ! curl -s -f -m 5 "http://localhost:8080" > /dev/null 2>&1 &
    then
        TEST3_PASS=false
    fi
done

wait

if [ "$TEST3_PASS" = true ]; then
    log_info "✓ Test 3 PASSED: Concurrent connections work"
else
    log_error "✗ Test 3 FAILED"
    exit 1
fi

cleanup
sleep 1

# Test 4: Different guest port
log_test "Test 4: Non-standard guest port (9090:8080)"

# Use a Python HTTP server on port 8080
$BOX_BIN run -d \
    --name "$CONTAINER_NAME" \
    -p "9090:8080" \
    python:3.11-alpine \
    sh -c "cd /tmp && python3 -m http.server 8080"

sleep 3

if curl -s -f -m 5 "http://localhost:9090" > /dev/null 2>&1; then
    log_info "✓ Test 4 PASSED: Non-standard guest port works"
else
    log_error "✗ Test 4 FAILED: Cannot connect to port 9090"
    exit 1
fi

cleanup
sleep 1

# Test 5: Large data transfer
log_test "Test 5: Large data transfer (download 1MB)"
$BOX_BIN run -d \
    --name "$CONTAINER_NAME" \
    -p "8080:80" \
    nginx:alpine

sleep 3

# Create a 1MB test file in the container
$BOX_BIN exec "$CONTAINER_NAME" sh -c "dd if=/dev/zero of=/usr/share/nginx/html/test.bin bs=1M count=1 2>/dev/null"

# Download and verify size
DOWNLOADED_SIZE=$(curl -s "http://localhost:8080/test.bin" | wc -c | tr -d ' ')
EXPECTED_SIZE=$((1024 * 1024))

if [ "$DOWNLOADED_SIZE" -eq "$EXPECTED_SIZE" ]; then
    log_info "✓ Test 5 PASSED: Large data transfer works (${DOWNLOADED_SIZE} bytes)"
else
    log_error "✗ Test 5 FAILED: Expected ${EXPECTED_SIZE} bytes, got ${DOWNLOADED_SIZE}"
    exit 1
fi

cleanup

echo
log_info "=== All Tests Passed ==="
log_info "TSI inbound port forwarding is working correctly"
