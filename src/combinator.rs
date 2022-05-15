use std::future::Future;

use std::mem::ManuallyDrop;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use pin_project::pin_project;
use crate::future::{AbortSafeFuture, AsyncDrop};
use crate::helpers::pin_manually_drop_as_mut;

#[pin_project]
pub struct Compat<Fut> {
    #[pin]
    inner: Option<Fut>,
}

impl<Fut> Compat<Fut> {
    pub fn new(inner: Fut) -> Self {
        Self {
            inner: Some(inner),
        }
    }
}

pub fn pending<T>() -> Compat<futures::future::Pending<T>> {
    Compat::new(futures::future::pending())
}

pub fn ready<T>(t: T) -> Compat<futures::future::Ready<T>> {
    Compat::new(futures::future::ready(t))
}


/// 所有Future都是abort safe的
impl<Fut: Future> AbortSafeFuture for Compat<Fut> {
    type Output = <Fut as Future>::Output;

    fn poll(mut self: Pin<&mut ManuallyDrop<Self>>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = pin_manually_drop_as_mut(&mut self).project();

        let output = if let Some(fut) = this.inner.as_mut().as_pin_mut() {
            ready!(fut.poll(cx))
        } else {
            panic!("Compat::poll called after completion or after canceled")
        };

        // drop inner future
        this.inner.set(None);
        Poll::Ready(output)
    }
}

impl<Fut> AsyncDrop for Compat<Fut> {
    fn poll_drop(mut self: Pin<&mut ManuallyDrop<Self>>, _cx: &mut Context<'_>) -> Poll<()> {
        let mut this = pin_manually_drop_as_mut(&mut self).project();
        // drop inner future
        this.inner.set(None);
        Poll::Ready(())
    }
}

#[pin_project]
pub struct Then<Fut1, Fut2, F>
where
    Fut1: AbortSafeFuture,
    Fut2: AbortSafeFuture,
{
    #[pin]
    inner: ThenInner<Fut1, Fut2>,
    f: Option<F>,
}


#[pin_project(project = ThenProj)]
enum ThenInner<Fut1, Fut2>
where
    Fut1: AbortSafeFuture,
    Fut2: AbortSafeFuture,
{
    Fut1(#[pin] ManuallyDrop<Fut1>, Option<Fut1::Output>),
    Fut2(#[pin] ManuallyDrop<Fut2>, Option<Fut2::Output>),
    Done,
    Canceled,
}

impl<Fut1, Fut2, F> Then<Fut1, Fut2, F>
where
    Fut1: AbortSafeFuture,
    Fut2: AbortSafeFuture,
{
    pub fn new(fut1: Fut1, f: F) -> Self {
        Self {
            inner: ThenInner::Fut1(ManuallyDrop::new(fut1), None),
            f: Some(f),
        }
    }
}

impl<Fut1, Fut2, F> AbortSafeFuture for Then<Fut1, Fut2, F>
where
    Fut1: AbortSafeFuture,
    Fut2: AbortSafeFuture,
    F: FnOnce(Fut1::Output) -> Fut2,
{
    type Output = Fut2::Output;

    fn poll(mut self: Pin<&mut ManuallyDrop<Self>>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = pin_manually_drop_as_mut(&mut self).project();
        let inner = this.inner.as_mut().project();
        match inner {
            ThenProj::Fut1(fut1, tmp @ None) => {
                *tmp = Some(ready!(fut1.poll(cx)));
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            ThenProj::Fut1(fut1, tmp @ Some(_)) => {
                ready!(fut1.poll_drop(cx));
                let f = this.f.take().expect("f was None, AndThen::poll may called after canceled");
                let tmp = tmp.take().unwrap();
                this.inner.set(ThenInner::Fut2(ManuallyDrop::new(f(tmp)), None));
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            ThenProj::Fut2(fut2, tmp @ None) => {
                *tmp = Some(ready!(fut2.poll(cx)));
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            ThenProj::Fut2(fut2, output @ Some(_)) => {
                ready!(fut2.poll_drop(cx));
                let output = output.take().unwrap();
                this.inner.set(ThenInner::Done);
                Poll::Ready(output)
            }
            ThenProj::Done => panic!("AndThen::poll called after completion"),
            ThenProj::Canceled => panic!("AndThen::poll called after canceled"),
        }
    }


}

impl<Fut1, Fut2, F> AsyncDrop for Then<Fut1, Fut2, F>
where
    Fut1: AbortSafeFuture,
    Fut2: AbortSafeFuture
{
    fn poll_drop(mut self: Pin<&mut ManuallyDrop<Self>>, cx: &mut Context<'_>) -> Poll<()> {
        let mut this = pin_manually_drop_as_mut(&mut self).project();
        // drop `f`
        let _ = this.f.take();

        let inner = this.inner.as_mut().project();
        match inner {
            ThenProj::Fut1(fut1, output) => {
                let _ = output.take();
                fut1.poll_drop(cx)
            }
            ThenProj::Fut2(fut2, output) => {
                let _ = output.take();
                fut2.poll_drop(cx)
            }
            ThenProj::Done => {
                this.inner.set(ThenInner::Canceled);
                Poll::Ready(())
            }
            ThenProj::Canceled => {
                Poll::Ready(())
            }
        }

    }
}