#!/bin/bash
# Set up build environment for astrbot_rs
export PATH="/tmp/host-bin:/tmp/toolchain/usr/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/home/ubuntu/.cargo/bin"
export CC="x86_64-linux-gnu-gcc-14"
export CFLAGS="--sysroot=/tmp/toolchain"
export LD_LIBRARY_PATH="/tmp/toolchain/usr/lib/x86_64-linux-gnu:/tmp/toolchain/usr/lib:/tmp/toolchain/lib/x86_64-linux-gnu:/lib/x86_64-linux-gnu:/usr/lib/x86_64-linux-gnu"
echo "Build environment ready"
