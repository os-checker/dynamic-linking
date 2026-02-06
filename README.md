# Rust 动态链接与异步共享库

## 背景

Rust 默认是静态链接的，所有用到的代码打包进最终的二进制可执行文件中，这样提供的文件可以直接一键部署和运行，无需设置依赖环境。

但 Rust 也支持动态链接。动态链接的核心价值是内存级共享复用、部署级解耦更新。

对于异步来说，动态链接可以让一个系统（插件系统、操作系统）中的不同模块共享代码，让异步任务被统一调度。

Rust 社区一直希望 tokio 异步库能够以共享库的方式工作（见 [#6927]），并支持异步 FFI（见 [RFC]）。
tokio 维护者对异步 FFI 的态度是技术上可在 tokio 外部实现和维护，当有广泛应用和发展势头可考虑合并到上游。

[#6927]: https://github.com/tokio-rs/tokio/issues/6927

[RFC]: https://github.com/tokio-rs/tokio/pull/6780

这个仓库探索动态链接和 tokio 异步共享库可以如何工作。

这些理解是我自己的，内容也不是 AI 生成的。有错误请见谅。

## tokio 异步共享库

社区发现 tokio 目前编译成共享库，单线程调度模型能够正常工作，但多线程模型则不能调度任务。

主要问题是，虽然 tokio 以共享库的方式工作，但运行时的内部状态（全局静态变量）由于符号修饰 (mangling) 存在副本，比如主程序动态链接了一个
tokio 库，调度器需要全局状态管理，静态变量使用一个符号，而其他动态加载的模块也依赖 tokio 库，但实际使用不同名的静态变量，从而主线程的运行时无法调度模块任务。

知名 Rust 博主 Amos 写了 [rubicon] 教程和库，花费 8 周调试，成功将 tokio 改成异步共享库。细节原理在 rubicon 仓库中已经被清晰地介绍。

[rubicon]: https://github.com/bearcove/rubicon

有两种方式加载共享库：

|          | 加载时机               | 如何加载                                   | 如何使用符号 (函数名、静态变量)                                           |
|----------|------------------------|--------------------------------------------|---------------------------------------------------------------------------|
| 自动加载 | 程序调用 main 函数之前 | 动态加载器查找并链接到 ELF 中声明的共享库  | 对于 Rust 共享库：模块路径语法引入；对于 C 共享库：使用 `extern "C"` 引入 |
| 手动加载 | 程序调用 main 函数之后 | 通过调用 `dlopen` 函数打开指定的共享库文件 | 以字符串方式手动引入符号，利用类型系统指定调用方式                        |

> 注：其实还有一种自动加载共享库的方式，在查找 ELF 的 `DT_NEEDED` 共享库声明之前，内核会把 vDSO 放入 link map 中。而
> vDSO 就是内核无条件在用户进程映射的某类特殊系统调用的共享库。
>
> 关于动态链接的工作流程见：[LWN: A look at dynamic linking (2024)](https://lwn.net/Articles/961117/)

Rubicon 作为一种 Rust 共享库的工作范式，做了以下事情：
* 用特定的宏包装线程局部变量（由 `thread_local!` 定义） 和进程局部变量 (由 `static` 定义)，保证它们不会被
  mangling、不被编译器优化掉，从而在整个进程和线程中保持全局共享、无副本。
* 对 cargo features 进行一些校验，保证以相同的 features 编译而避免 ABI 不兼容。

整个过程没有定义新的 FFI 接口，而是假设 ABI 兼容，避免未定义行为的安全风险，因此要求
* 模块只加载不卸载：保证 `'static` 的 Rust 语义。如果中途卸载模块，其异步任务可能正在运行，但共享库的状态和符号已经不存在。
* 使用相同版本的 Rust 编译器构建程序和所有库/模块：保证内存布局、调用约定等 ABI 兼容性。不同版本的编译器不保证 ABI 相同。
* 使用相同的 cargo features 进行编译：保证 ABI 兼容性。不同的编译标志会影响一个数据结构的内存布局，比如有的字段被条件编译，那么模块之间使用“同名的”数据结构，但二进制表示不同。

对 tokio 生态的基础库调整成上面的 rubicon 方式，仅需将 `static` 和 `thread_local!` 的符号名替换成未修饰（为了不导致全局冲突，填充了 crate 名），并以推荐的方式编译程序和模块，那么就能正常工作。

## 异步 FFI

如果整个系统只有 Rust 代码，那么 rubicon 范式完全不需要 FFI 就能工作。因为 Rust 动态库 (dylib) 可以被自动加载，也可以被手动加载。

但如果你需要某个模块与非 Rust 代码交互，或者模块是非 Rust 编写的，那么确定的 ABI 是必要的。

Rust 以两种方式提供稳定的 ABI：
* `repr(C)` 被放置在 struct/enum/union 上，并可能和其他修饰符一起控制其内存布局，其含义被 [Reference][c-layout] 指定。
* [`unsafe extern "C"`] 修饰函数，表示该函数遵循 C 的调用约定。

[c-layout]: https://doc.rust-lang.org/reference/type-layout.html#the-c-representation

一些具体的标准库的类型的布局可能被单独记录，比如 [`UnsafeCell`]、[`NonZero`]，因此 FFI 涉及的每个 Rust 类型都需要逐一确认布局情况。

[`UnsafeCell`]: https://doc.rust-lang.org/std/cell/struct.UnsafeCell.html#memory-layout
[`NonZero`]: https://doc.rust-lang.org/std/num/struct.NonZero.html

此外，[`crabi`] 也是一个将来的可能，它在 C ABI 的基础上提供一些高级类型的稳定的内存布局规范，但目前代码尚未合并到编译器内。

[`crabi`]: https://github.com/rust-lang/rust/pull/105586

FFI 的设计取决于传递什么，对于异步代码，通常传递用于唤醒回调事件，以及就绪的数据。

[这里][rust-async-ffi] AI 总结得很好。如果在 Rust 和 C 之间 FFI，有很多设计选择
* 通信机制：传统的回调、事件驱动、共享内存队列、消息队列。
* Rust 到 C 的数据结构生成：用 `#[repr(C)]` + `extern "C"`，配合 [cbindgen] 自动生成 C 头文件。

如果在 Rust 模块之间传递异步任务，那么有一系列的库可选：
* [async-ffi] 提供了稳定布局的 Future/Poll/Context，以及一个宏包装异步函数来返回这个 Future。
* [stabby] 和 [abi_stable] 提供完整的 Rust 类型稳定布局方案，包括 trait objects。
  * 比较：stabby 充分利用类型系统来保持 niche 布局，而 abi_stable 更成熟。
  * 它们都使用复杂的抽象，并且不适合生成相应的 C 数据结构。

[rust-async-ffi]: https://github.com/zjp-CN/os-notes/issues/1

[cbindgen]: https://github.com/mozilla/cbindgen

[async-ffi]: https://docs.rs/async-ffi
[stabby]: https://docs.rs/stabby/latest/stabby/future/index.html
[abi_stable]: https://docs.rs/abi_stable/latest/abi_stable/std_types/struct.RBox.html#impl-Future-for-RBox%3CF%3E

## dylib 不适合作为真正的共享库

cdylib 和 dylib 的介绍见 [Reference: Linkage](https://doc.rust-lang.org/reference/linkage.html)。

如果完全面向 Rust 程序，dylib 可以充当共享库，但 ABI 兼容是一个很大的风险，相同的编译器容易固定，但不同库的编译条件很难保证，
当依赖库的数据结构随编译条件而变化，那么我们必须使用 cdylib，并公布稳定的 ABI 进行 FFI，以保证安全性。

我们仍然可以尝试在异步操作系统中尝试基于 dylib 构建一个内部的小组件，因为它没有类型转换和 FFI 的复杂性，同时保持共享库的优势。

在本仓库的实验中，我还发现如下一些特点：
* cdylib 可以暴露非 `extern "C"` 的函数，并在 Rust 程序中正常执行：只需保证 ABI 兼容（相同编译器和编译条件等等）。
* dylib 可以被 libloading 动态加载：将 mod_a 从 cdylib 改为 dylib，整个代码能够正常工作，可能毕竟 dylib 也是标准的 ELF 文件。
* 面向 Rust 程序的依赖，cdylib 不能充当 dylib：将 exports 从 dylib 改为 cdylib，你会得到如下错误

```bash
error[E0432]: unresolved import `exports`
 --> src/main.rs:1:5
  |
1 | use exports::tokio;
  |     ^^^^^^^ use of unresolved module or unlinked crate `exports`
  |
  = help: if you wanted to use a crate named `exports`, use `cargo add exports` to add it to your `Cargo.toml`
```

## 零碎的记录

### Rust 的 Future 膨胀问题

嵌套的 Future 在体积上尚未优化，因此内存不必要地太大。这也导致编译产物也会膨胀，执行效率未达到最优。

即便开启 size 优化，LLVM 的效果也不太奏效。可以说，Future 在这方面不是零成本抽象。

需要在 Rust MIR 上对状态机进行内联优化，比如合并无 Pending 状态的 Future。

2026 年有两个项目目标致力于此问题：
* [Async Future Memory Optimisation](https://rust-lang.github.io/rust-project-goals/2026/async-future-memory-optimisation.html)
* [Async statemachine optimisation](https://rust-lang.github.io/rust-project-goals/2026/async-statemachine-optimisation.html)

### Miri 无法分析共享库

Miri 不能识别 Rust 动态库依赖：

```
rubicon/test-crates/samplebin $ cargo miri run
Preparing a sysroot for Miri (target: aarch64-unknown-linux-gnu)... done
error: cannot produce dylib for `exports v0.1.0 (./rubicon/test-crates/exports)` as the target
`aarch64-unknown-linux-gnu` does not support these crate types
```

但对于 C 共享库 (cdylib)，Miri 提供 `-Zmiri-native-lib=<path to a shared object file or folder>`
参数，支持 FFI 调用，但不支持 FFI 上的任何代码检查（使得内存分析存在 unsound 问题）。而且实际功能受限：比如仅限
Unix 系统、只支持整数和指针类型的参数和返回值。

相关链接：
* tracking issue: [Support native FFI calls via libffi](https://github.com/rust-lang/miri/issues/11)
* 最初的设计文档：[Miri C FFI Extension](https://hackmd.io/eFY7Jyl6QGeGKQlJvBp6pw)
* 改进 FFI 内存跟踪：[(more) precisely track memory accesses and allocations across FFI](https://github.com/rust-lang/miri/pull/4326)

### 查看共享库依赖情况

```bash
# app is an executable file
$ readelf -d target/debug/app | grep NEEDED
 0x0000000000000001 (NEEDED)             Shared library: [libexports.so]
 0x0000000000000001 (NEEDED)             Shared library: [libstd-f60440a8f78133a4.so]
 0x0000000000000001 (NEEDED)             Shared library: [libgcc_s.so.1]
 0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]

$ ldd target/debug/app
    linux-vdso.so.1 (0x0000fcadb484c000)
    libexports.so => not found
    libstd-f60440a8f78133a4.so => not found
    libgcc_s.so.1 => /lib/aarch64-linux-gnu/libgcc_s.so.1 (0x0000fcadb4700000)
    libc.so.6 => /lib/aarch64-linux-gnu/libc.so.6 (0x0000fcadb4540000)
    /lib/ld-linux-aarch64.so.1 (0x0000fcadb4810000)
```

```bash
# mod_a is a (rust) dylib
$ readelf -d target/debug/libexports.so | grep NEEDED
 0x0000000000000001 (NEEDED)             Shared library: [libstd-f60440a8f78133a4.so]
 0x0000000000000001 (NEEDED)             Shared library: [libgcc_s.so.1]
 0x0000000000000001 (NEEDED)             Shared library: [libm.so.6]
 0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]

$ ldd target/debug/libexports.so 
    linux-vdso.so.1 (0x0000f00e06d3c000)
    libstd-f60440a8f78133a4.so => not found
    libgcc_s.so.1 => /lib/aarch64-linux-gnu/libgcc_s.so.1 (0x0000f00e06af0000)
    libm.so.6 => /lib/aarch64-linux-gnu/libm.so.6 (0x0000f00e06a40000)
    libc.so.6 => /lib/aarch64-linux-gnu/libc.so.6 (0x0000f00e06880000)
    /lib/ld-linux-aarch64.so.1 (0x0000f00e06cf0000)
```

```bash
# mod_a is a cdylib
$ readelf -d mod_a/target/debug/libmod_a.so | grep NEEDED
 0x0000000000000001 (NEEDED)             Shared library: [libgcc_s.so.1]
 0x0000000000000001 (NEEDED)             Shared library: [libm.so.6]
 0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]
```

### 体积测试

动态链接由于其共享性，可以很大程度地减少二进制文件的体积。这里是一些编译产物的体积数据：

| elf           | size (`--release`) | size (`--debug`) |
|---------------|--------------------|------------------|
| app           | 326K               | 6M               |
| libexports.so | 833K               | 7M               |
| libmod_a.so   | 580K               | 10M              |

| elf         | crate-type | `RUSTFLAGS`                      | size (`--release`) | size (`--debug`) |
|-------------|------------|----------------------------------|--------------------|------------------|
| libmod_a.so | cdylib     |                                  | 580K               | 10M              |
| libmod_a.so | cdylib     | `-Cprefer-dynamic`               | 201K               | -                |
| libmod_a.so | cdylib     | `-Cprefer-dynamic -Copt-level=s` | 229K               | -                |
| libmod_a.so | dylib      |                                  | 2.5M               | 14M              |
| libmod_a.so | dylib      | `-Cprefer-dynamic`               | 878K               | -                |
| libmod_a.so | dylib      | `-Cprefer-dynamic -Copt-level=s` | 1.1M               | -                |

| elf           | crate-type | `RUSTFLAGS`        | size (`--release`) | size (`--debug`) |
|---------------|------------|--------------------|--------------------|------------------|
| app           | bin        |                    | 326K               | 6M               |
| app           | bin        | `-Cprefer-dynamic` | 326K               | -                |
| app           | bin        | `-Copt-level=s`    | 354K               | -                |
| libexports.so | dylib      |                    | 833K               | 7M               |
| libexports.so | dylib      | `-Cprefer-dynamic` | 833K               | -                |
| libexports.so | dylib      | `-Copt-level=s`    | 1008K              | -                |

我们可以看到 cdylib 比 dylib 小很多，[`-Cprefer-dynamic`] 可以进一步减小体积。

[`-Cprefer-dynamic`]: https://doc.rust-lang.org/rustc/codegen-options/index.html#prefer-dynamic

