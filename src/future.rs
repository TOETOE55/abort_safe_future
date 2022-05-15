use std::task::Context;
use std::task::Poll;
use std::pin::Pin;
use std::mem::ManuallyDrop;

use crate::combinator::Then;
use crate::helpers::pin_manually_drop_as_mut;


/// abort safe future
///
// FIXME: 是否需要AbortSafeFuture: AsyncDrop这个约束?
pub trait AbortSafeFuture: AsyncDrop {
    /// Future的结果类型
    type Output;
    /// 与std中的Future类似。
    /// 但需要注意：
    ///
    /// * 调用者无法析构`Self`，需要实现者自己实现资源的回收。
    /// * 完成或中断子future时，需要调用其`poll_drop`方法，直到其返回`Poll::Ready`。否则可能内存泄漏。
    fn poll(self: Pin<&mut ManuallyDrop<Self>>, cx: &mut Context<'_>) -> Poll<Self::Output>;
}

pub trait AsyncDrop {

    /// 当Future成功或者需要中断之后调用，进行一些资源回收的工作。
    ///
    // FIXME: 是否下面这个签名更合适？
    // ```rust
    // unsafe fn poll_drop(self: *mut self, cx: &mut Context<'_>) -> Poll<()>;
    // ```
    fn poll_drop(self: Pin<&mut ManuallyDrop<Self>>, cx: &mut Context<'_>) -> Poll<()>;
}

// FIXME: 这个实现是否合理? 违背了`poll`完需要`poll_drop`的约定
impl<F: AbortSafeFuture + Unpin + ?Sized> AbortSafeFuture for &mut ManuallyDrop<F> {
    type Output = F::Output;

    fn poll(mut self: Pin<&mut ManuallyDrop<Self>>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        F::poll(Pin::new(&mut*self), cx)
    }
}

impl<F: AsyncDrop + Unpin + ?Sized> AsyncDrop for &mut ManuallyDrop<F> {
    fn poll_drop(mut self: Pin<&mut ManuallyDrop<Self>>, cx: &mut Context<'_>) -> Poll<()> {
        F::poll_drop(Pin::new(&mut*self), cx)
    }
}

// FIXME: 这个实现是否合理? 违背了`poll`完需要`poll_drop`的约定
impl<F: AbortSafeFuture + ?Sized> AbortSafeFuture for Pin<&mut ManuallyDrop<F>> {
    type Output = F::Output;

    fn poll(mut self: Pin<&mut ManuallyDrop<Self>>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let inner = pin_manually_drop_as_mut(&mut self).as_deref_mut();
        F::poll(inner, cx)
    }
}

impl<F: AsyncDrop + ?Sized> AsyncDrop for Pin<&mut ManuallyDrop<F>> {
    fn poll_drop(mut self: Pin<&mut ManuallyDrop<Self>>, cx: &mut Context<'_>) -> Poll<()> {
        let inner = pin_manually_drop_as_mut(&mut self).as_deref_mut();
        F::poll_drop(inner, cx)
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
