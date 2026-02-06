#!/bin/bash

set -eoux pipefail

# export RUSTFLAGS="-Cprefer-dynamic"

pushd mod_a
cargo clean
cargo b
mod_a=$PWD/target/debug
popd

pushd mod_c
gcc mod_c.c -o libmod_c.so -fPIC -shared -pthread
mod_c=$PWD
popd

cargo r
app=$PWD/target/debug

# export LD_LIBRARY_PATH=$(rustc --print sysroot)/lib/rustlib/aarch64-unknown-linux-gnu/lib:$mod_a:$mod_c:$app
# valgrind --leak-check=full "$app"/app
