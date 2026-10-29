# Source this file in the managed cloud environment.
export CARGO_HOME="${CARGO_HOME:-/workspace/.cargo}"
export RUSTUP_HOME="${RUSTUP_HOME:-/workspace/.rustup}"
export PATH="/workspace/.rusted-tools/bin:$CARGO_HOME/bin:$PATH"
