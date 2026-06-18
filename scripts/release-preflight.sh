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

# When updating the osxcross-base image (new macOS SDK), update these two
# variables to match the new darwin version and SDK name.
DARWIN_VERSION="darwin25.5"
MACOS_SDK="MacOSX26.5.sdk"

echo "Checking osxcross toolchain (${DARWIN_VERSION}, ${MACOS_SDK})..."
echo "  Installed clang targets: $(ls /opt/osxcross/target/bin/*-clang 2>/dev/null | xargs -n1 basename | tr '\n' ' ' || echo '(none)')"
for tool in clang ar ld ranlib; do
  bin="/opt/osxcross/target/bin/aarch64-apple-${DARWIN_VERSION}-${tool}"
  test -x "$bin" || { echo "FAIL: missing osxcross tool: $bin"; echo "  Hint: update DARWIN_VERSION in this script to match the osxcross-base image tag"; exit 1; }
done
test -d "/opt/osxcross/target/SDK/${MACOS_SDK}" \
  || { echo "FAIL: missing SDK dir: /opt/osxcross/target/SDK/${MACOS_SDK}"; echo "  Hint: update MACOS_SDK in this script to match the osxcross-base image"; exit 1; }

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
