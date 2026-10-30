# Source this file in the managed cloud environment.
export CARGO_HOME="${CARGO_HOME:-/workspace/.cargo}"
export RUSTUP_HOME="${RUSTUP_HOME:-/workspace/.rustup}"
export PATH="/workspace/.rusted-tools/bin:$CARGO_HOME/bin:$PATH"
# Development headers installed in the managed workspace (ALSA and udev).
if [ -d /workspace/.rusted-tools/horizon-linux/usr ]; then
    export PKG_CONFIG_PATH="/workspace/.rusted-tools/horizon-linux/usr/lib/x86_64-linux-gnu/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
    export CPATH="/workspace/.rusted-tools/horizon-linux/usr/include${CPATH:+:$CPATH}"
fi
