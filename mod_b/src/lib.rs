#[async_ffi::async_ffi]
#[unsafe(no_mangle)]
pub async fn async_add(a: i32, b: i32) -> i32 {
    a + b
}
