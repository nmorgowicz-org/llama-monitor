#!/bin/bash
# Audits #[cfg(target_os = "macos")] module guards for consistency.
# Checks that modules gated behind cfg(macos) aren't imported unconditionally
# in non-gated code.

set -e

cd "$(dirname "$0")/.."

echo "=== cfg(target_os = \"macos\") audit ==="

errors=0

# Find all modules gated behind cfg(target_os = "macos")
while IFS= read -r gate_line; do
    # Extract the module name from the next line (handles multi-line cfg attributes)
    mod_name=$(echo "$gate_line" | sed -n 's/.*pub mod \([a-z_]*\);.*/\1/p')
    mod_name=$(echo "$mod_name" | head -1)

    # If no module name found on same line, check next line
    if [ -z "$mod_name" ]; then
        mod_name=$(echo "$gate_line" | awk '{print $NF}' | sed 's/pub //;s/ //;s/;')
    fi

    [ -z "$mod_name" ] && continue

    # Check if this module is imported unconditionally elsewhere in src/
    while IFS= read -r usage_file; do
        # Check if the import is guarded by same cfg
        line_before=$(grep -n "use.*::${mod_name}" "$usage_file" | head -1 | cut -d: -f1)
        if [ -n "$line_before" ]; then
            prev_line=$((line_before - 1))
            if [ "$prev_line" -gt 0 ]; then
                guard=$(sed -n "${prev_line}p" "$usage_file")
                if ! echo "$guard" | grep -q "cfg.*macos"; then
                    echo "⚠ Unconditional import of macOS-gated module '${mod_name}' in ${usage_file}:${line_before}"
                    errors=$((errors + 1))
                fi
            fi
        fi
    done < <(grep -rl "use.*${mod_name}" src/ 2>/dev/null || true)
done < <(grep -rn "cfg(target_os = \"macos\")" src/ | grep "pub mod")

if [ "$errors" -gt 0 ]; then
    echo "FAILED: Found $errors cfg guard violations"
    exit 1
fi

echo "✓ All cfg(target_os = \"macos\") guards are consistent"
exit 0
