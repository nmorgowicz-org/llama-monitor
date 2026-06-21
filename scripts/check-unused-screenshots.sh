#!/usr/bin/env bash
# Check which screenshots are not referenced in any docs.
# Usage: bash scripts/check-unused-screenshots.sh
set -euo pipefail
cd "$(dirname "$0")/.."

FILES=$(find docs/screenshots -maxdepth 1 -type f \( -name '*.png' -o -name '*.gif' -o -name '*.webp' \) | sed 's|^docs/screenshots/||' | sort)

if [ -z "$FILES" ]; then
  echo "No screenshot files found."
  exit 0
fi

# Search entire repo for each filename (markdown images, HTML img, plain references)
echo "=== UNREFERENCED screenshots ==="
UNREF=""
for f in $FILES; do
  # grep for the filename in any file; exclude the screenshot itself
  if ! git grep -l "$f" -- ':!docs/screenshots/*' > /dev/null 2>&1; then
    UNREF="$UNREF $f"
    echo "  $f"
  fi
done

if [ -z "$UNREF" ]; then
  echo "(none — all screenshots are referenced)"
else
  echo ""
  echo "Tip: Promote to docs/screenshots/ only after adding an image reference in your docs."
fi
