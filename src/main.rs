use exports::{
    DynFut, Fn, Fut, FutBuffer,
    dynify::{Dynify, PinConstruct},
    tokio,
};
use std::mem::MaybeUninit;
use std::pin::Pin;

#[tokio::main(worker_threads = 4)]
async fn main() {
    run_mod_a().await;

    std::thread::sleep(std::time::Duration::new(1, 0));
    println!("done");
}

async fn run_mod_a() {
    let mod_a = unsafe { libloading::Library::new("./mod_a/target/debug/libmod_a.so").unwrap() };
    println!("mod_a is loaded");
    let mod_a = Box::leak(Box::new(mod_a));

    let run = unsafe { mod_a.get::<unsafe extern "C" fn()>(b"run\0").unwrap() };
    println!("run is got");
    unsafe { run() };
    println!("run is running");

    tokio::spawn(async { println!("😎 Task from main.") });
    println!("main task is spawned");

    let task = unsafe {
        *mod_a
            .get::<unsafe fn() -> Pin<Box<dyn 'static + Send + Future<Output = ()>>>>(b"task\0")
            .unwrap()
    };
    println!("task is got");
    tokio::spawn(async move { unsafe { task().await } });
    println!("task is running");

    let hello = unsafe {
        *mod_a
            .get::<unsafe fn() -> Fut<String>>(b"async_hello\0")
            .unwrap()
    };
    tokio::spawn(async move {
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
    });
    println!("hello is running");
    tokio::spawn(async move {
        let mut buf = FutBuffer::<16>::new();
        dbg!(
            unsafe { hello() }.init(&mut buf).await,
            buf.spilled(),
            buf.capacity(),
            buf.len()
        );
    });

    let take_string = unsafe {
        *mod_a
            .get::<unsafe fn(String) -> Fn!(String => DynFut<String>)>(b"take_string\0")
            .unwrap()
    };
    tokio::spawn(async move {
        let mut stack = [MaybeUninit::<u8>::uninit(); 32];
        let mut heap = Vec::<MaybeUninit<u8>>::new();
        let fut_take_string =
            unsafe { take_string("hello".to_owned()) }.init2(&mut stack, &mut heap);
        dbg!(fut_take_string.await);
    });
    tokio::spawn(async move {
        let mut buf = FutBuffer::<32>::new();
        dbg!(
            unsafe { take_string("hi".to_owned()) }.init(&mut buf).await,
            buf.spilled(),
            buf.capacity(),
            buf.len()
        );
    });

    let concat = unsafe {
        *mod_a
            .get::<unsafe fn(String, String) -> Fn!(String, String => DynFut<String>)>(b"concat\0")
            .unwrap()
    };
    tokio::spawn(async move {
        let mut stack = [MaybeUninit::<u8>::uninit(); 32];
        let mut heap = Vec::<MaybeUninit<u8>>::new();
        let concat =
            unsafe { concat("hello".to_owned(), " world".to_owned()) }.init2(&mut stack, &mut heap);
        dbg!(concat.await);
    });
}
