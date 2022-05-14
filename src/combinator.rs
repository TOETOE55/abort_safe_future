use std::future::Future;

use std::mem::ManuallyDrop;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use pin_project::pin_project;
use crate::future::AbortSafeFuture;
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
            panic!("Compat::poll called after completion or after cancel")
        };

        // drop inner future
        this.inner.set(None);
        Poll::Ready(output)
    }

    fn poll_cancel(mut self: Pin<&mut ManuallyDrop<Self>>, _cx: &mut Context<'_>) -> Poll<()> {
        let mut this = pin_manually_drop_as_mut(&mut self).project();
        // drop inner future
        this.inner.set(None);
        Poll::Ready(())
    }
}

#[pin_project]
pub struct Then<Fut1, Fut2, F> {
    #[pin]
    inner: ThenInner<Fut1, Fut2>,
    f: Option<F>,
}


#[pin_project(project = ThenProj)]
enum ThenInner<Fut1, Fut2> {
    Fut1(#[pin] ManuallyDrop<Fut1>),
    Fut2(#[pin] ManuallyDrop<Fut2>),
    Done,
    Canceled,
}

impl<Fut1, Fut2, F> Then<Fut1, Fut2, F> {
    pub fn new(fut1: Fut1, f: F) -> Self {
        Self {
            inner: ThenInner::Fut1(ManuallyDrop::new(fut1)),
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
            ThenProj::Fut1(fut1) => {
                let output = ready!(fut1.poll(cx));
                let f = this.f.take().unwrap();
                this.inner.set(ThenInner::Fut2(ManuallyDrop::new(f(output))));
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            ThenProj::Fut2(fut2) => {
                let output = ready!(fut2.poll(cx));
                this.inner.set(ThenInner::Done);
                Poll::Ready(output)
            }
            ThenProj::Done => panic!("AndThen::poll called after completion"),
            ThenProj::Canceled => panic!("AndThen::poll called after cancel"),
        }
    }

    fn poll_cancel(mut self: Pin<&mut ManuallyDrop<Self>>, cx: &mut Context<'_>) -> Poll<()> {
        let mut this = pin_manually_drop_as_mut(&mut self).project();
        let inner = this.inner.as_mut().project();
        match inner {
            ThenProj::Fut1(fut1) => {
                fut1.poll_cancel(cx)
            }
            ThenProj::Fut2(fut2) => {
                fut2.poll_cancel(cx)
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