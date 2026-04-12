#!/bin/bash
# Test mactop JSON output structure
# Save as test-mactop.sh and run on macOS with: ./test-mactop.sh

echo "=== mactop JSON Schema Test ==="
echo "Date: $(date)"
echo ""

# Check if mactop is installed
if ! command -v mactop &> /dev/null; then
    echo "ERROR: mactop not found. Install with: brew install mactop"
    exit 1
fi

echo "mactop version: $(mactop --version 2>&1 || echo 'unknown')"
echo ""

# Run mactop headless mode
echo "=== Full JSON Output ==="
mactop --headless --count 1 --format json 2>/dev/null

echo ""
echo "=== Key Fields to Look For ==="
echo ""
echo "1. GPU-related fields:"
echo "   - gpu_usage, gpu_temp, gpu_power, gpu_clock"
echo "   - gpu_memory, vram_used, vram_total, memory_used, memory_total"
echo ""
echo "2. Power fields:"
echo "   - cpu_power, gpu_power, ane_power, system_power"
echo ""
echo "3. Memory fields:"
echo "   - memory_used, memory_total, memory_percent"
echo "   - dram_read_bandwidth, dram_write_bandwidth"
echo ""
echo "4. Fan fields:"
echo "   - fans, fan_rpm, fan_target"
echo ""
echo "5. Disk/Network fields:"
echo "   - disk_read, disk_write, network_up, network_down"

echo ""
echo "=== Raw JSON (pipe to jq for formatting) ==="
echo "If jq is installed, try: mactop --headless --count 1 --format json | jq ."
