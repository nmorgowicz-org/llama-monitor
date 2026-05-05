#!/bin/bash
# run-server.sh — wrapper for Playwright webServer that ensures a fresh config dir.
# Creates a temp config directory, passes it to the app, and cleans up on exit.

set -e

TEST_CONFIG_DIR=$(mktemp -d /tmp/llama-monitor-test-XXXXXX)
echo "$TEST_CONFIG_DIR" > /tmp/llama-monitor-test-config-path
echo "[run-server] Fresh config dir: $TEST_CONFIG_DIR"

cleanup() {
    echo "[run-server] Cleaning up: $TEST_CONFIG_DIR"
    rm -rf "$TEST_CONFIG_DIR"
    rm -f /tmp/llama-monitor-test-config-path
}
trap cleanup EXIT

exec cargo run -- --headless --port 7778 --config-dir "$TEST_CONFIG_DIR"
