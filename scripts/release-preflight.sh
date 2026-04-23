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

test -x /opt/osxcross/target/bin/aarch64-apple-darwin25.1-clang
test -x /opt/osxcross/target/bin/aarch64-apple-darwin25.1-ar
test -x /opt/osxcross/target/bin/aarch64-apple-darwin25.1-ld
test -x /opt/osxcross/target/bin/aarch64-apple-darwin25.1-ranlib
test -d /opt/osxcross/target/SDK/MacOSX26.1.sdk

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
