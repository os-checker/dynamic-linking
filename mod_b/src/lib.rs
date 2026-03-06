#[async_ffi::async_ffi]
#[unsafe(no_mangle)]
pub async fn async_add(a: i32, b: i32) -> i32 {
    a + b
}

#[async_ffi::async_ffi]
#[unsafe(no_mangle)]
pub async fn sleep(secs: u64) {
    println!("sleep {secs} secs: start");
    std::thread::sleep(std::time::Duration::from_secs(secs));
    println!("sleep {secs} secs: end");
}
