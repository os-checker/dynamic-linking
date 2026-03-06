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

#[async_ffi::async_ffi]
#[unsafe(no_mangle)]
pub async fn sleep_with_pending(secs: u64) {
    println!("sleep_with_pending {secs} secs: start");

    let mut polled = false;
    std::future::poll_fn(|cx| {
        if polled {
            println!("sleep_with_pending: polled Ready");
            core::task::Poll::Ready(())
        } else {
            polled = true;
            println!("sleep_with_pending: polled Pending");
            cx.waker().wake_by_ref();
            core::task::Poll::Pending
        }
    })
    .await;
    println!("sleep_with_pending: poll_fn finishes");

    std::thread::sleep(std::time::Duration::from_secs(secs));
    println!("sleep_with_pending {secs} secs: end");
}
