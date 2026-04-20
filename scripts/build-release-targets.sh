#!/usr/bin/env bash
set -euo pipefail

mkdir -p ~/.cargo
cat > ~/.cargo/config.toml << 'EOF'
[target.aarch64-unknown-linux-gnu]
linker = "aarch64-linux-gnu-gcc"
rustflags = ["-C", "link-arg=-Wl,--allow-shlib-undefined"]

[target.aarch64-apple-darwin]
linker = "/opt/osxcross/target/bin/aarch64-apple-darwin25.1-clang"
ar = "/opt/osxcross/target/bin/aarch64-apple-darwin25.1-ar"
rustflags = [
  "-C", "link-arg=-fuse-ld=/opt/osxcross/target/bin/aarch64-apple-darwin25.1-ld",
  "-C", "link-arg=-isysroot",
  "-C", "link-arg=/opt/osxcross/target/SDK/MacOSX26.1.sdk",
]
EOF

cargo build --release --target x86_64-unknown-linux-gnu &
pid1=$!

PKG_CONFIG_ALLOW_CROSS=1 \
  PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig \
  cargo build --release --target aarch64-unknown-linux-gnu &
pid2=$!

cargo build --release --target x86_64-pc-windows-gnu &
pid3=$!

SDKROOT=/opt/osxcross/target/SDK/MacOSX26.1.sdk \
  CC_aarch64_apple_darwin=/opt/osxcross/target/bin/aarch64-apple-darwin25.1-clang \
  AR_aarch64_apple_darwin=/opt/osxcross/target/bin/aarch64-apple-darwin25.1-ar \
  cargo build --release --target aarch64-apple-darwin &
pid4=$!

result=0
wait "$pid1" || result=1
wait "$pid2" || result=1
wait "$pid3" || result=1
wait "$pid4" || result=1
exit "$result"

