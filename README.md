遗留的问题
* 宏展开错误

```rust
#[tokio::main(worker_threads = 4)] ● failed to write request: The length of a sequence must be known
```


* 内存访问错误

https://docs.rs/stabby/latest/stabby/future/
https://docs.rs/async-ffi/
https://github.com/rodrimati1992/abi_stable_crates

https://doc.rust-lang.org/reference/linkage.html


[LWN: A look at dynamic linking (2024)](https://en.wikipedia.org/wiki/Dynamic_linker)

## Miri 无法分析动态库

```
rubicon/test-crates/samplebin $ cargo miri run
Preparing a sysroot for Miri (target: aarch64-unknown-linux-gnu)... done
error: cannot produce dylib for `exports v0.1.0 (./rubicon/test-crates/exports)` as the target
`aarch64-unknown-linux-gnu` does not support these crate types
```
