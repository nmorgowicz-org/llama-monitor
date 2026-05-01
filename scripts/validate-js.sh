#!/bin/bash
# Validate all JS files before committing
# Run this before pushing to catch syntax errors and missing dependencies

set -e

echo "=== Step 1: Validate syntax as ES module ==="
for f in static/js/features/*.js static/js/core/*.js static/js/bootstrap.js static/js/compat/*.js static/js/init-state.js; do
    if [ -f "$f" ]; then
        cp "$f" /tmp/test.mjs
        if ! node --check /tmp/test.mjs 2>&1; then
            echo "FAIL: $f"
            exit 1
        fi
    fi
done
echo "✓ All files pass syntax check"

echo ""
echo "=== Step 2: Check for missing window.* references ==="
# Find all window.* calls in feature modules that aren't on window
for f in static/js/features/*.js; do
    if [ -f "$f" ]; then
        # Find all window.* calls
        grep -oE 'window\.[a-zA-Z]+' "$f" | sort -u | while read ref; do
            func=$(echo "$ref" | sed 's/window\.//')
            # Check if this function is on window from any module
            if ! grep -q "window\.$func = " static/js/features/*.js static/js/compat/*.js 2>/dev/null; then
                # Check if it's a DOM API (document, navigator, etc.)
                if echo "$func" | grep -qE '^(document|navigator|location|history|screen|localStorage|sessionStorage|setTimeout|setInterval|clearTimeout|clearInterval|requestAnimationFrame|cancelAnimationFrame|addEventListener|removeEventListener|dispatchEvent|querySelector|querySelectorAll|getElementById|getElementsByClassName|getElementsByTagName|createElement|createTextNode|appendChild|insertBefore|removeChild|replaceChild|console|alert|confirm|prompt|fetch|WebSocket|XMLHttpRequest|URL|URLSearchParams|Blob|File|FileReader|FormData|Headers|Request|Response|ArrayBuffer|Int8Array|Uint8Array|Float32Array|Float64Array|Map|Set|WeakMap|WeakSet|Promise|Symbol|Proxy|Reflect|Intl|Math|JSON|RegExp|Error|TypeError|ReferenceError|SyntaxError|RangeError|URIError|Object|Array|String|Number|Boolean|Date|parseInt|parseFloat|isNaN|isFinite|encodeURI|decodeURI|encodeURIComponent|decodeURIComponent|escape|unescape|eval|isNaN|isFinite|Infinity|NaN|undefined|null|true|false|getSelection|matchMedia)$'; then
                    continue
                fi
                echo "⚠ $f: $ref not on window (may be missing)"
            fi
        done
    fi
done
echo "✓ Window reference check complete"

echo ""
echo "=== All checks passed ==="
