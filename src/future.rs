use core::task::Context;
use core::task::Poll;
use core::pin::Pin;
use core::mem::ManuallyDrop;

use std::any::type_name;
use std::task::ready;
use crate::combinator::Then;
use crate::helpers::pin_manually_drop_as_mut;


/// abort safe future
pub trait AbortSafeFuture {
    /// Future的结果类型
    type Output;
    /// 与std中的Future类似。
    /// 但需要注意：
    ///
    /// * 调用者无法析构`Self`，需要实现者自己实现资源的回收。
    /// * 中断子future时，需要调用其`poll_cancel`方法，直到其返回`Poll::Ready`。否则可能内存泄漏。
    fn poll(self: Pin<&mut ManuallyDrop<Self>>, cx: &mut Context<'_>) -> Poll<Self::Output>;

    /// 当被取消时，调用此方法。
    /// 返回值为`Poll::Ready`时，表示取消成功，此时应该完成了一些资源的回收工作
    fn poll_cancel(self: Pin<&mut ManuallyDrop<Self>>, cx: &mut Context<'_>) -> Poll<()>;
}

impl<F: AbortSafeFuture + Unpin + ?Sized> AbortSafeFuture for &mut ManuallyDrop<F> {
    type Output = F::Output;

    fn poll(mut self: Pin<&mut ManuallyDrop<Self>>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        F::poll(Pin::new(&mut*self), cx)
    }

    fn poll_cancel(mut self: Pin<&mut ManuallyDrop<Self>>, cx: &mut Context<'_>) -> Poll<()> {
        F::poll_cancel(Pin::new(&mut*self), cx)
    }
}

impl<F: AbortSafeFuture + Unpin + ?Sized> AbortSafeFuture for Option<Box<ManuallyDrop<F>>> {
    type Output = F::Output;

    fn poll(mut self: Pin<&mut ManuallyDrop<Self>>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this: &mut Option<_> = &mut*self;
        if let Some(fut) = this {
            let output = ready!(F::poll(Pin::new(fut), cx));
            // drop box is safe
            unsafe {
                ManuallyDrop::drop(fut);
            }
            *this = None;

            Poll::Ready(output)
        } else {
            panic!("`{} poll after completion or after cancelled`", type_name::<Self>())
        }
    }

    fn poll_cancel(mut self: Pin<&mut ManuallyDrop<Self>>, cx: &mut Context<'_>) -> Poll<()> {
        let this: &mut Option<_> = &mut*self;
        if let Some(fut) = this {
            ready!(F::poll_cancel(Pin::new(fut), cx));
            // drop box is safe
            unsafe {
                ManuallyDrop::drop(fut);
            }
            *this = None;
        }

        Poll::Ready(())
    }
}

impl<F: AbortSafeFuture + ?Sized> AbortSafeFuture for Pin<&mut ManuallyDrop<F>> {
    type Output = F::Output;

    fn poll(mut self: Pin<&mut ManuallyDrop<Self>>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let inner = pin_manually_drop_as_mut(&mut self).as_deref_mut();
        F::poll(inner, cx)
    }

    fn poll_cancel(mut self: Pin<&mut ManuallyDrop<Self>>, cx: &mut Context<'_>) -> Poll<()> {
        let inner = pin_manually_drop_as_mut(&mut self).as_deref_mut();
        F::poll_cancel(inner, cx)
    }
}

impl<F: AbortSafeFuture + ?Sized> AbortSafeFuture for Option<Pin<Box<ManuallyDrop<F>>>> {
    type Output = F::Output;

    fn poll(mut self: Pin<&mut ManuallyDrop<Self>>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut inner = pin_manually_drop_as_mut(&mut self);
        if let Some(fut) = inner.as_mut().as_pin_mut().as_deref_mut() {
            let output = ready!(F::poll(fut.as_mut(), cx));
            // drop box is safe
            unsafe {
                ManuallyDrop::drop(fut.as_mut().get_unchecked_mut());
            }
            inner.set(None);

            Poll::Ready(output)
        } else {
            panic!("`{} poll after completion or after cancelled`", type_name::<Self>())
        }

    }

    fn poll_cancel(mut self: Pin<&mut ManuallyDrop<Self>>, cx: &mut Context<'_>) -> Poll<()> {
        let mut inner = pin_manually_drop_as_mut(&mut self);
        if let Some(fut) = inner.as_mut().as_pin_mut().as_deref_mut() {
            ready!(F::poll_cancel(fut.as_mut(), cx));
            // drop box is safe
            unsafe {
                ManuallyDrop::drop(fut.as_mut().get_unchecked_mut());
            }
            inner.set(None);
        }

        Poll::Ready(())
    }
}


pub trait AbortSafeFutureExt: AbortSafeFuture {
    fn then<Fut, F>(self, f: F) -> Then<Self, Fut, F>
    where
        Self: Sized,
        Fut: AbortSafeFuture,
        F: FnOnce(Self::Output) -> Fut,
    {
        Then::new(self, f)
    }
}

impl<Fut: AbortSafeFuture> AbortSafeFutureExt for Fut {}
