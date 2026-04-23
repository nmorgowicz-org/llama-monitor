#!/usr/bin/env bash
set -euo pipefail

mkdir -p ~/.cargo
cat > ~/.cargo/config.toml << 'EOF'
[target.aarch64-apple-darwin]
linker = "/opt/osxcross/target/bin/aarch64-apple-darwin25.1-clang"
ar = "/opt/osxcross/target/bin/aarch64-apple-darwin25.1-ar"
rustflags = [
  "-C", "link-arg=-fuse-ld=/opt/osxcross/target/bin/aarch64-apple-darwin25.1-ld",
  "-C", "link-arg=-isysroot",
  "-C", "link-arg=/opt/osxcross/target/SDK/MacOSX26.1.sdk",
]
EOF

echo "Building release targets..."

cargo build --release --target x86_64-unknown-linux-gnu \
  --target-dir target/smoke-x86_64-linux \
  > /tmp/build-linux-x86_64.log 2>&1 &
pid1=$!

# CROSS_REMOTE=1: dind's Docker daemon can't bind-mount /opt/rustup from the
# runner container, so cross copies the sysroot via docker cp instead.
CROSS_REMOTE=1 \
  CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C link-arg=-Wl,--allow-shlib-undefined" \
  cross build --release --target aarch64-unknown-linux-gnu \
  --target-dir target/smoke-aarch64-linux \
  > /tmp/build-linux-aarch64.log 2>&1 &
pid2=$!

CROSS_REMOTE=1 \
  CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C target-feature=+crt-static" \
  cross build --release --target x86_64-pc-windows-gnu \
  --target-dir target/smoke-x86_64-windows \
  --no-default-features --features native-tray \
  > /tmp/build-windows-x86_64.log 2>&1 &
pid3=$!

SDKROOT=/opt/osxcross/target/SDK/MacOSX26.1.sdk \
  CC_aarch64_apple_darwin=/opt/osxcross/target/bin/aarch64-apple-darwin25.1-clang \
  AR_aarch64_apple_darwin=/opt/osxcross/target/bin/aarch64-apple-darwin25.1-ar \
  AR=/opt/osxcross/target/bin/aarch64-apple-darwin25.1-ar \
  RANLIB=/opt/osxcross/target/bin/aarch64-apple-darwin25.1-ranlib \
  cargo build --release --target aarch64-apple-darwin \
  --target-dir target/smoke-aarch64-macos \
  > /tmp/build-macos-aarch64.log 2>&1 &
pid4=$!

result=0

wait "$pid1" || { echo "FAILED: x86_64-unknown-linux-gnu"; cat /tmp/build-linux-x86_64.log; result=1; }
wait "$pid2" || { echo "FAILED: aarch64-unknown-linux-gnu"; cat /tmp/build-linux-aarch64.log; result=1; }
wait "$pid3" || { echo "FAILED: x86_64-pc-windows-gnu"; cat /tmp/build-windows-x86_64.log; result=1; }
wait "$pid4" || { echo "FAILED: aarch64-apple-darwin"; cat /tmp/build-macos-aarch64.log; result=1; }

[ "$result" -eq 0 ] && echo "All release targets built successfully."
exit "$result"
