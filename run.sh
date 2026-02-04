#!/bin/bash

set -eoux pipefail

pushd mod_a
cargo clean
cargo b
mod_a=$PWD/target/debug
popd
cargo r
app=$PWD/target/debug

# export LD_LIBRARY_PATH=$(rustc --print sysroot)/lib/rustlib/aarch64-unknown-linux-gnu/lib:$mod_a:$app
# valgrind --leak-check=full "$app"/app
