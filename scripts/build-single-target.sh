#!/usr/bin/env bash
# Build one release target. Called by the matrix release workflow.
# Usage: build-single-target.sh <target>
set -euo pipefail

TARGET="${1:?Usage: build-single-target.sh <target>}"

mkdir -p ~/.cargo
cat > ~/.cargo/config.toml << 'CARGO_CONFIG'
[target.aarch64-apple-darwin]
linker = "/opt/osxcross/target/bin/aarch64-apple-darwin25.1-clang"
ar     = "/opt/osxcross/target/bin/aarch64-apple-darwin25.1-ar"
rustflags = [
  "-C", "link-arg=-fuse-ld=/opt/osxcross/target/bin/aarch64-apple-darwin25.1-ld",
  "-C", "link-arg=-isysroot",
  "-C", "link-arg=/opt/osxcross/target/SDK/MacOSX26.1.sdk",
]

[target.aarch64-unknown-linux-gnu]
linker = "aarch64-linux-gnu-gcc"
rustflags = [
  "-C", "link-arg=-Wl,--allow-shlib-undefined",
]
CARGO_CONFIG

case "$TARGET" in
  x86_64-unknown-linux-gnu)
    cargo build --release --target x86_64-unknown-linux-gnu
    ;;
  aarch64-unknown-linux-gnu)
    PKG_CONFIG_ALLOW_CROSS=1 \
      PKG_CONFIG_LIBDIR=/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig \
      CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
      CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++ \
      AR_aarch64_unknown_linux_gnu=aarch64-linux-gnu-ar \
      cargo build --release --target aarch64-unknown-linux-gnu
    ;;
  x86_64-pc-windows-gnu)
    CROSS_REMOTE=1 \
      CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C target-feature=+crt-static" \
      cross build --release --target x86_64-pc-windows-gnu \
      --no-default-features --features native-tray
    ;;
  aarch64-apple-darwin)
    SDKROOT=/opt/osxcross/target/SDK/MacOSX26.1.sdk \
      CC_aarch64_apple_darwin=/opt/osxcross/target/bin/aarch64-apple-darwin25.1-clang \
      AR_aarch64_apple_darwin=/opt/osxcross/target/bin/aarch64-apple-darwin25.1-ar \
      AR=/opt/osxcross/target/bin/aarch64-apple-darwin25.1-ar \
      RANLIB=/opt/osxcross/target/bin/aarch64-apple-darwin25.1-ranlib \
      cargo build --release --target aarch64-apple-darwin
    ;;
  *)
    echo "ERROR: unknown target '$TARGET'" >&2
    exit 1
    ;;
esac
