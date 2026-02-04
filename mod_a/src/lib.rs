#[unsafe(no_mangle)]
pub extern "C" fn run() {
    println!("mod_a run starts");
    tokio::spawn(async { println!("😎 Task from mod_a") });
    println!("mod_a task is spawned");
}

#[unsafe(no_mangle)]
pub fn task() -> std::pin::Pin<Box<dyn Send + Future<Output = ()>>> {
    Box::pin(async { println!("🎉 An async task!") })
}

use dynify::{Fn, from_fn};

#[unsafe(no_mangle)]
fn async_hello() -> Fn!(=> dyn Future<Output = String>) {
    from_fn!(|| async {
        async {}.await;
        let a = [b'a'; 1024];
        std::str::from_utf8(&a).unwrap().to_owned()
    })
}

struct S;
impl S {
    #[unsafe(no_mangle)]
    fn task2() -> Fn!(=> dyn Future<Output = String>) {
        from_fn!(|| async {
            async {}.await;
            let a = [b'a'; 1024];
            std::str::from_utf8(&a).unwrap().to_owned()
        })
    }
}
