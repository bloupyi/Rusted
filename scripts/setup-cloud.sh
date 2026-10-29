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
rustup toolchain install 1.99.1 --profile minimal
cargo build --release --locked -p rusted-launch -j 4
cargo test --workspace --locked -j 4
python3 scripts/test_horizon_bridge.py
python3 scripts/rusted.py verify
cargo +1.99.1 test --manifest-path engine/Cargo.toml --locked -p rusted-platform
printf '%s\n' 'Rusted launcher bridge, metadata diagnostics and source provenance validated. Build the native game with python scripts/rusted.py build; see docs/NATIVE-WINDOWS.md.'
