use exports::async_ffi::FfiFuture;
use exports::{tokio, tokio::task::JoinSet};

#[tokio::main(worker_threads = 4)]
async fn main() {
    let mut set = JoinSet::new();

    // run_mod_a().await;
    run_mod_b(&mut set);
    run_mod_c(&mut set);

    set.join_all().await;

    println!("done");
}

fn spawn<F: Future<Output = ()> + Send + 'static>(fut: F, set: &mut JoinSet<()>) {
    set.spawn(fut);
}

fn run_mod_a(set: &mut JoinSet<()>) {
    use exports::{
        DynFut, Fn, Fut, FutBuffer,
        dynify::{Dynify, PinConstruct},
    };
    use std::mem::MaybeUninit;
    use std::pin::Pin;

    let mod_a = unsafe { libloading::Library::new("./mod_a/target/debug/libmod_a.so").unwrap() };
    println!("mod_a is loaded");
    let mod_a = Box::leak(Box::new(mod_a));

    let run = unsafe { mod_a.get::<unsafe extern "C" fn()>(b"run\0").unwrap() };
    println!("run is got");
    unsafe { run() };
    println!("run is running");

    spawn(async { println!("😎 Task from main.") }, set);
    println!("main task is spawned");

    let task = unsafe {
        *mod_a
            .get::<unsafe fn() -> Pin<Box<dyn 'static + Send + Future<Output = ()>>>>(b"task\0")
            .unwrap()
    };
    println!("task is got");
    spawn(async move { unsafe { task().await } }, set);
    println!("task is running");

    let task = unsafe {
        *mod_a
            .get::<fn() -> Pin<Box<dyn 'static + Send + Future<Output = ()>>>>(b"task\0")
            .unwrap()
    };
    println!("task is got again");
    spawn(task(), set);
    println!("task is running again");

    let hello = unsafe {
        *mod_a
            .get::<unsafe fn() -> Fut<String>>(b"async_hello\0")
            .unwrap()
    };
    spawn(
        async move {
            let mut stack = [MaybeUninit::<u8>::uninit(); 16];
            let mut heap = Vec::<MaybeUninit<u8>>::new();
            let hello = unsafe { hello() };
            dbg!(hello.layout());
            match hello.try_init(&mut stack) {
                Ok(fut) => _ = dbg!(fut.await),
                Err((this, _)) => {
                    println!("Initialized on the heap");
                    match this.try_init(&mut heap) {
                        Ok(fut) => _ = dbg!(fut.await),
                        Err(_) => panic!("Failed to init on heap"),
                    }
                }
            }
            dbg!(heap.len(), heap.capacity());
        },
        set,
    );
    println!("hello is running");
    spawn(
        async move {
            let mut buf = FutBuffer::<16>::new();
            dbg!(
                unsafe { hello() }.init(&mut buf).await,
                buf.spilled(),
                buf.capacity(),
                buf.len()
            );
        },
        set,
    );

    let take_string = unsafe {
        *mod_a
            .get::<unsafe fn(String) -> Fn!(String => DynFut<String>)>(b"take_string\0")
            .unwrap()
    };
    spawn(
        async move {
            let mut stack = [MaybeUninit::<u8>::uninit(); 32];
            let mut heap = Vec::<MaybeUninit<u8>>::new();
            let fut_take_string =
                unsafe { take_string("hello".to_owned()) }.init2(&mut stack, &mut heap);
            dbg!(fut_take_string.await);
        },
        set,
    );
    spawn(
        async move {
            let mut buf = FutBuffer::<32>::new();
            dbg!(
                unsafe { take_string("hi".to_owned()) }.init(&mut buf).await,
                buf.spilled(),
                buf.capacity(),
                buf.len()
            );
        },
        set,
    );

    let concat = unsafe {
        *mod_a
            .get::<unsafe fn(String, String) -> Fn!(String, String => DynFut<String>)>(b"concat\0")
            .unwrap()
    };
    spawn(
        async move {
            let mut stack = [MaybeUninit::<u8>::uninit(); 32];
            let mut heap = Vec::<MaybeUninit<u8>>::new();
            let concat = unsafe { concat("hello".to_owned(), " world".to_owned()) }
                .init2(&mut stack, &mut heap);
            dbg!(concat.await);
        },
        set,
    );
}

type Task = extern "C" fn(u32, u32) -> FfiFuture<u32>;

fn run_mod_c(set: &mut JoinSet<()>) {
    let mod_c = unsafe { libloading::Library::new("./mod_c/libmod_c.so").unwrap() };
    println!("mod_c is loaded");
    let mod_c = Box::leak(Box::new(mod_c));

    let async_add: Task = unsafe { *mod_c.get(b"async_add\0").unwrap() };
    spawn(
        async move {
            println!("[mod_c] async_add: {}", async_add(2, 0).await);
        },
        set,
    );
}

fn run_mod_b(set: &mut JoinSet<()>) {
    let mod_b = unsafe { libloading::Library::new("./mod_b/target/debug/libmod_b.so").unwrap() };
    println!("mod_b is loaded");
    let mod_b = Box::leak(Box::new(mod_b));

    let async_add: Task = unsafe { *mod_b.get(b"async_add\0").unwrap() };
    spawn(
        async move {
            println!("[mod_b] async_add: {}", async_add(1, 0).await);
        },
        set,
    );

    let sleep: fn(u64) -> FfiFuture<()> = unsafe { *mod_b.get(b"sleep\0").unwrap() };
    spawn(sleep(3), set);
}
