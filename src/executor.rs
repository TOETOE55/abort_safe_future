use std::mem::ManuallyDrop;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake};
use std::thread;
use std::thread::Thread;
use crate::future::AbortSafeFuture;

struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

pub fn block_on<T>(fut: impl AbortSafeFuture<Output = T>) -> T {
    let mut fut = Box::pin(ManuallyDrop::new(fut)) as Pin<Box<ManuallyDrop<dyn AbortSafeFuture<Output = T>>>>;

    let t = thread::current();
    let waker = Arc::new(ThreadWaker(t)).into();
    let mut cx = Context::from_waker(&waker);

    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(res) => return res,
            Poll::Pending => thread::park(),
        }
    }
}