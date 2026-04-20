#!/usr/bin/env bash
set -euo pipefail

command -v cargo >/dev/null
command -v rustup >/dev/null
command -v pkg-config >/dev/null
command -v aarch64-linux-gnu-gcc >/dev/null
command -v x86_64-w64-mingw32-gcc >/dev/null

test -x /opt/osxcross/target/bin/aarch64-apple-darwin25.1-clang
test -x /opt/osxcross/target/bin/aarch64-apple-darwin25.1-ar
test -x /opt/osxcross/target/bin/aarch64-apple-darwin25.1-ld
test -d /opt/osxcross/target/SDK/MacOSX26.1.sdk

dpkg -s gcc-aarch64-linux-gnu >/dev/null
dpkg -s gcc-mingw-w64-x86-64 >/dev/null
dpkg -s libwebkit2gtk-4.1-dev:arm64 >/dev/null
dpkg -s libsoup-3.0-dev:arm64 >/dev/null
dpkg -s libjavascriptcoregtk-4.1-dev:arm64 >/dev/null
dpkg -s libayatana-appindicator3-dev:arm64 >/dev/null
dpkg -s libgtk-3-dev:arm64 >/dev/null

export PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig

pkg-config --libs webkit2gtk-4.1 >/dev/null
pkg-config --libs javascriptcoregtk-4.1 >/dev/null
pkg-config --libs libsoup-3.0 >/dev/null

