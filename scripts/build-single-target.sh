#!/usr/bin/env bash
# Build one release target. Called by the matrix release workflow.
# Usage: build-single-target.sh <target>
set -euo pipefail

TARGET="${1:?Usage: build-single-target.sh <target>}"

# Auto-detect from the installed osxcross toolchain — no manual updates needed.
DARWIN_VERSION=$(ls /opt/osxcross/target/bin/aarch64-apple-darwin*-clang 2>/dev/null \
  | head -1 | grep -oP 'darwin[\d.]+')
MACOS_SDK=$(ls -d /opt/osxcross/target/SDK/MacOSX*.sdk 2>/dev/null \
  | sort -V | tail -1 | xargs basename)
if [[ -z "$DARWIN_VERSION" || -z "$MACOS_SDK" ]]; then
  echo "FAIL: could not detect osxcross toolchain in /opt/osxcross/target/"
  echo "  clang binaries found: $(ls /opt/osxcross/target/bin/*-clang 2>/dev/null || echo none)"
  echo "  SDKs found: $(ls -d /opt/osxcross/target/SDK/*.sdk 2>/dev/null || echo none)"
  exit 1
fi
echo "osxcross: ${DARWIN_VERSION}, ${MACOS_SDK}"

mkdir -p ~/.cargo
cat > ~/.cargo/config.toml << CARGO_CONFIG
[target.aarch64-apple-darwin]
linker = "/opt/osxcross/target/bin/aarch64-apple-${DARWIN_VERSION}-clang"
ar     = "/opt/osxcross/target/bin/aarch64-apple-${DARWIN_VERSION}-ar"
rustflags = [
  "-C", "link-arg=-fuse-ld=/opt/osxcross/target/bin/aarch64-apple-${DARWIN_VERSION}-ld",
  "-C", "link-arg=-isysroot",
  "-C", "link-arg=/opt/osxcross/target/SDK/${MACOS_SDK}",
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
    SDKROOT="/opt/osxcross/target/SDK/${MACOS_SDK}" \
      CC_aarch64_apple_darwin="/opt/osxcross/target/bin/aarch64-apple-${DARWIN_VERSION}-clang" \
      AR_aarch64_apple_darwin="/opt/osxcross/target/bin/aarch64-apple-${DARWIN_VERSION}-ar" \
      AR="/opt/osxcross/target/bin/aarch64-apple-${DARWIN_VERSION}-ar" \
      RANLIB="/opt/osxcross/target/bin/aarch64-apple-${DARWIN_VERSION}-ranlib" \
      cargo build --release --target aarch64-apple-darwin
    ;;
  *)
    echo "ERROR: unknown target '$TARGET'" >&2
    exit 1
    ;;
esac
