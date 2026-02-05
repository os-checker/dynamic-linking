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
type Fut<T = ()> = Fn!(=> dyn Send + Future<Output = T>);

#[unsafe(no_mangle)]
fn async_hello() -> Fut<String> {
    from_fn!(|| async {
        async {}.await;
        let a = [b'a'; 1024];
        std::str::from_utf8(&a).unwrap().to_owned()
    })
}

struct S;
impl S {
    #[unsafe(no_mangle)]
    fn task2() -> Fut<String> {
        from_fn!(|| async {
            async {}.await;
            let a = [b'a'; 1024];
            std::str::from_utf8(&a).unwrap().to_owned()
        })
    }
}

#[unsafe(no_mangle)]
fn take_string(s: String) -> Fn!(String => dyn Send + Future<Output = String>) {
    async fn inner(mut s: String) -> String {
        s.push_str(" world");
        s
    }

    from_fn!(inner, s)
}

#[unsafe(no_mangle)]
fn concat(a: String, b: String) -> Fn!(String, String => dyn Send + Future<Output = String>) {
    async fn inner(a: String, b: String) -> String {
        a + &b
    }
    from_fn!(inner, a, b)
}
