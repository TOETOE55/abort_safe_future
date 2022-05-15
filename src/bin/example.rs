use abort_safe_future::AbortSafeFutureExt;
use abort_safe_future::combinator::Compat;
use abort_safe_future::executor::block_on;

struct TestDrop;

impl Drop for TestDrop {
    fn drop(&mut self) {
        println!("`TestDrop` is dropping")
    }
}

fn main() {
    block_on(Compat::new(async {
        let _guard = TestDrop;
        async {}.await;
        println!("Hello, world!");
    }).then(|_| Compat::new(async {
        println!("Goodbye, world!");
    })));
}
