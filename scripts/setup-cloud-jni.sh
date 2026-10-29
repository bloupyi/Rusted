#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/cloud-env.sh
mkdir -p /workspace/.rusted-tools
if ! command -v rustup >/dev/null; then
    (
        cd /workspace/.rusted-tools
        curl -fsSLo rustup-init https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init
        curl -fsSLo rustup-init.sha256 https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init.sha256
        sha256sum -c rustup-init.sha256
        chmod +x rustup-init
        ./rustup-init -y --no-modify-path --profile minimal --default-toolchain 1.90.0
    )
fi
python3 scripts/prepare-jdk.py
cargo build --release --locked -j 4
cargo test --workspace --locked -j 4
python3 scripts/test_bridge.py --no-build
native_flags=(--linux target/release/librusted_backend.so)
if [ -f target/x86_64-pc-windows-gnu/release/rusted_backend.dll ]; then
    native_flags+=(--windows target/x86_64-pc-windows-gnu/release/rusted_backend.dll)
fi
python3 scripts/build_mod.py "${native_flags[@]}"
printf '%s\n' 'Native bridge validated and Fabric JAR built. Run scripts/test_minecraft.py for real Minecraft codec integration.'
