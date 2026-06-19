#!/usr/bin/env bash
set -euo pipefail

echo "Running release preflight checks..."

command -v cargo >/dev/null || { echo "FAIL: cargo not found"; exit 1; }
command -v rustup >/dev/null || { echo "FAIL: rustup not found"; exit 1; }
command -v cross >/dev/null || { echo "FAIL: cross not found"; exit 1; }
command -v docker >/dev/null || { echo "FAIL: docker not found"; exit 1; }
command -v pkg-config >/dev/null || { echo "FAIL: pkg-config not found"; exit 1; }
command -v aarch64-linux-gnu-gcc >/dev/null || { echo "FAIL: aarch64-linux-gnu-gcc not found"; exit 1; }
command -v aarch64-linux-gnu-g++ >/dev/null || { echo "FAIL: aarch64-linux-gnu-g++ not found"; exit 1; }

docker info >/dev/null 2>&1 || { echo "FAIL: Docker daemon not reachable"; exit 1; }
docker buildx version >/dev/null 2>&1 || { echo "FAIL: docker buildx plugin not available"; exit 1; }

# Auto-detect from the installed osxcross toolchain — no manual updates needed.
DARWIN_VERSION=$(ls /opt/osxcross/target/bin/aarch64-apple-darwin*-clang 2>/dev/null \
  | head -1 | grep -oP 'darwin[\d.]+')
MACOS_SDK=$(ls -d /opt/osxcross/target/SDK/MacOSX*.sdk 2>/dev/null \
  | sort -V | tail -1 | xargs basename)
if [[ -z "$DARWIN_VERSION" || -z "$MACOS_SDK" ]]; then
  echo "FAIL: could not detect osxcross toolchain in /opt/osxcross/target/"
  echo "  clang binaries: $(ls /opt/osxcross/target/bin/*-clang 2>/dev/null || echo none)"
  echo "  SDKs: $(ls -d /opt/osxcross/target/SDK/*.sdk 2>/dev/null || echo none)"
  exit 1
fi

echo "Checking osxcross toolchain (${DARWIN_VERSION}, ${MACOS_SDK})..."
for tool in clang ar ld ranlib; do
  bin="/opt/osxcross/target/bin/aarch64-apple-${DARWIN_VERSION}-${tool}"
  test -x "$bin" || { echo "FAIL: missing osxcross tool: $bin"; exit 1; }
done
test -d "/opt/osxcross/target/SDK/${MACOS_SDK}" \
  || { echo "FAIL: missing SDK dir: /opt/osxcross/target/SDK/${MACOS_SDK}"; exit 1; }

for target in \
  x86_64-unknown-linux-gnu \
  aarch64-unknown-linux-gnu \
  x86_64-pc-windows-gnu \
  aarch64-apple-darwin; do
  rustup target list --installed | grep -q "^${target}$" \
    || { echo "FAIL: rustup target ${target} not installed"; exit 1; }
done

pkg-config --libs webkit2gtk-4.1 >/dev/null
pkg-config --libs javascriptcoregtk-4.1 >/dev/null
pkg-config --libs libsoup-3.0 >/dev/null

PKG_CONFIG_ALLOW_CROSS=1 \
  PKG_CONFIG_LIBDIR=/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig \
  pkg-config --libs webkit2gtk-4.1 >/dev/null
PKG_CONFIG_ALLOW_CROSS=1 \
  PKG_CONFIG_LIBDIR=/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig \
  pkg-config --libs javascriptcoregtk-4.1 >/dev/null
PKG_CONFIG_ALLOW_CROSS=1 \
  PKG_CONFIG_LIBDIR=/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig \
  pkg-config --libs libsoup-3.0 >/dev/null

echo "All preflight checks passed."
