---
title: Rust 动态链接与异步共享库
titleTemplate: '%s'
info: Rust 动态链接与异步共享库
author: 周积萍
date: 2026-03-05
theme: seriph
# background: https://picsum.photos/800/600
transition: fade
routerMode: hash
# download: true
monaco: false
hideInToc: true
---

<h1 class="font-bold !text-orange-500">Rust 动态链接与异步共享库</h1>

[仓库](https://github.com/os-checker/dynamic-linking)

<style scoped>
.slidev-layout.cover {
  background: var(--slidev-theme-background) !important;
  color: var(--slidev-theme-foreground) !important;
}
</style>

---

# 加载动态库

```rust
let mod_a = unsafe { libloading::Library::new("./mod_a/target/debug/libmod_a.so").unwrap() };
```

* 自动加载：程序调用 main 函数之前
  * 内核无条件在用户进程映射 vDSO
  * 动态加载器查找并链接到 ELF 中声明的共享库
* 手动加载：程序调用 main 函数之后
  * 调用 dlopen 函数打开指定的共享库文件

---

# 调用普通函数

```rust
let run = unsafe { mod_a.get::<unsafe extern "C" fn()>(b"run\0").unwrap() };
unsafe { run() };
```

* [`Library::get`] 是 unsafe 函数：需保证函数签名类型正确
* `unsafe extern "C" fn()` 是自己引入的签名，需要自己保证 ABI
  * 类型签名上的 unsafe 修饰符不是必须，你可以这么写：

```rust
let run = unsafe { mod_a.get::</* 不声明为 unsafe */ extern "C" fn()>(b"run\0").unwrap() };
run();
```

[`Library::get`]: https://docs.rs/libloading/latest/libloading/struct.Library.html#method.get

---

# 调用异步函数

对同一份代码使用相同版本的编译器和编译参数，那么直接 ABI 兼容：

```rust
// app:
let task = unsafe {
    *mod_a
        .get::<fn() -> Pin<Box<dyn 'static + Send + Future<Output = ()>>>>(b"task\0")
        .unwrap()
};

tokio::spawn(task());
tokio::spawn(async move { task().await });
```

```rust
// mod_a:
#[unsafe(no_mangle)]
pub fn task() -> std::pin::Pin<Box<dyn Send + Future<Output = ()>>> {
    Box::pin(async { println!("🎉 An async task!") })
}
```

---

# 异步共享库

* [Rubicon](https://github.com/bearcove/rubicon) 范式
  * 动机：让 tokio 异步库能够以共享库的方式工作
  * 核心问题：对于运行时的内部状态（比如全局变量），保证程序和共享库的符号一致
  * 工作方式：
    * 用特定的宏包装线程局部变量（由 thread_local! 定义） 和进程局部变量 (由 static 定义)，保证它们不会被 mangling、不被编译器优化掉，从而在整个进程和线程中保持全局共享、无副本
    * 对 cargo features 进行一些校验，保证以相同的 features 编译而避免 ABI 不兼容
    * 整个过程没有定义新的 FFI 接口，而是假设 ABI 兼容
* [dylib vs cdylib](https://github.com/os-checker/dynamic-linking?tab=readme-ov-file#dylib-%E4%B8%8D%E9%80%82%E5%90%88%E4%BD%9C%E4%B8%BA%E7%9C%9F%E6%AD%A3%E7%9A%84%E5%85%B1%E4%BA%AB%E5%BA%93)

---

# 调用异步 FFI 的函数

```rust
// app:
let async_add: Task = unsafe { *mod_b.get(b"async_add\0").unwrap() };
async_add(1, 2).await;
type Task = unsafe extern "C" fn(i32, i32) -> async_ffi::FfiFuture<i32>;
```

FfiFuture 是 C ABI 兼容的，并且在任何 Rust 版本中都保持 ABI 一致：

## 示例: Rust 编写的模块

```rust
#[async_ffi::async_ffi]
#[unsafe(no_mangle)]
pub async fn async_add(a: i32, b: i32) -> i32 {
    a + b
}
```

---

## 示例: C 编写的模块

```c
struct FfiWakerVTable {
    struct FfiWaker const *(*clone)(struct FfiWaker const *);
    void (*wake)(struct FfiWaker const *);
    void (*wake_by_ref)(struct FfiWaker const *);
    void (*drop)(struct FfiWaker const *);
};

struct FfiWaker { struct FfiWakerVTable const *vtable; };

struct FfiContext { struct FfiWaker const *waker_ref; };

struct PollU32 { uint8_t is_pending; union { uint32_t value; }; };

struct FfiFutureU32 {
    void *fut;
    struct PollU32 (*poll)(void *fut, struct FfiContext *context);
    void (*drop)(void *fut);
};
```

---

```c
struct my_data {
    uint32_t state;
    uint32_t a, b, ret;
    pthread_t handle;
    struct FfiWaker const *waker;
};

struct FfiFutureU32 async_add (uint32_t a, uint32_t b) {
    struct my_data *data = malloc(sizeof(struct my_data));
    data->handle = 0;
    data->state = 0;
    data->a = a;
    data->b = b;
    struct FfiFutureU32 fut = {
        .fut = (void *)data,
        .poll = fut_poll,
        .drop = fut_drop,
    };
    return fut;
}
```

---

# 异步 FFI

[笔记](https://github.com/os-checker/dynamic-linking?tab=readme-ov-file#%E5%BC%82%E6%AD%A5-ffi)

* Why：异步数据结构需要保证明确的 ABI 吗？
  * 内部细节可能无需保证 ABI
  * 但对外接口需要稳定的 ABI

---

* How：FFI 的设计取决于传递什么，对于异步代码，通常传递用于唤醒回调事件，以及就绪的数据
  * 通信机制：传统的回调、事件驱动、共享内存队列、消息队列
  * 内存布局方案
    * 基本思路
      * `#[repr(C)]` 等修饰符控制内存布局
      * `unsafe extern "C" fn` 表示该函数遵循 C 的调用约定
      * [crabi] 在 C ABI 的基础上提供一些高级类型的稳定的内存布局规范，但目前代码尚未合并到编译器内
    * 封装库
      * async-ffi 提供了稳定布局的 Future/Poll/Context，以及一个宏包装异步函数来返回这个 Future
      * stabby 和 abi_stable 提供完整的 Rust 类型稳定布局方案，包括 trait objects

[crabi]: https://github.com/rust-lang/rust/pull/105586
