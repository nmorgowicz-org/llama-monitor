#!/bin/bash
# Validate all JavaScript files for syntax errors
# Runs node --check on each .js file to catch syntax errors before browser load

set -e

echo "Validating JavaScript files..."

FAILED=0

# Use find to catch all JS files at any depth (glob ** requires globstar in bash)
while IFS= read -r file; do
    # Node.js treats .js as CommonJS by default, but our files use ES module syntax
    # Copy to .mjs to validate as ES module (matching how the browser treats them)
    cp "$file" /tmp/test-esm.mjs

    if node --check /tmp/test-esm.mjs 2>/dev/null; then
        echo "✓ $file"
    else
        echo "✗ $file - SYNTAX ERROR"
        node --check /tmp/test-esm.mjs 2>&1 | head -5
        FAILED=1
    fi

    rm -f /tmp/test-esm.mjs
done < <(find static/js -name "*.js" -type f | sort)

if [ $FAILED -eq 1 ]; then
    echo ""
    echo "JavaScript validation FAILED. Fix syntax errors before committing."
    exit 1
fi

echo ""
echo "All JavaScript files validated successfully."
