# AbortSafeFuture

std中的Future可以通过随时来取消/中断。比如`select!`/`timeout`，会在一定条件下直接中断inner Future。
对于inner Future来说，就像是在.await处发生了panic，然后unwind the stack。
但这种中断Future本身是无法感知到的，不能像同步上下文那样子`catch_unwind`。
这不是一个好的设计。

所以这个新的api目标之一，是*让用户可以感知并处理Future的中断*。


Future中断的原理是析构，而`poll`的参数是`Pin<&mut Self>`，我们随时可以中断掉一个Future，无论一个Future处在什么状态。
并且，`poll`是safe的，这就要求任意的Future，在任意状态都能被安全的析构。
这个条件其实是很强的，其实有很多异步的操作，都不满足这个条件。

比如下面的scoped task，假如`foo`产生的Future被中断了，字符串`s`就会被析构掉。但spawn的task不会被中断，这时候就会产生UB。

```rust
async fn foo() {
    let s = String::from("hello");
    
    task::scoped(|scp| async {
        scp.spawn(async {
            sleep(Duration::from_secs(10)).await;
            println!("{}", s);
        });
    }).await;
}
```

除了scoped task，其实也有不少异步的库因为要满足“随时可安全析构”的条件而难以设计（而且大概率有bug）。

所以这个新的api目标之二，是*让Future不可以随时被析构*。

## API

```rust
pub trait AbortSafeFuture {
    type Output;
    fn poll(self: Pin<&mut ManuallyDrop<Self>>, cx: &mut Context<'_>) -> Poll<Self::Output>;
    fn poll_cancel(self: Pin<&mut ManuallyDrop<Self>>, cx: &mut Context<'_>) -> Poll<()>;
}
```

为了满足目标一，相比于std的Future，我们多提供了一个`poll_cancel`的方法，在需要中断的时候调用。如果不调用，就可能产生内存泄露（而非UB）。
（有点类似于`AsyncDrop`要做的事情）。

而为了满足目标二，`poll_*`的参数是`self: Pin<&mut ManuallyDrop<Self>>`。
这样我们除了不能随意移动`Self`，也不能随意析构`Self`。

因为`Self`被`ManuallyDrop`和`Pin`保护了，所以`Self`自身的析构函数不会被调用（调用者无法析构`Self`）。
所以所有资源回收的操作都需要在`poll_*`中完成，并由开发者自己保证正确性。
比如说，我们没办法为任意`Pin<Box<ManuallyDrop<Fut: AbortSafeFuture>>>`实现`AbortSafeFuture`，
因为一旦手动回收了，再访问的时候`ManuallyDrop<Fut>`就会产生UB。

这个api与其他类似的[api](https://docs.rs/completion-core/0.2.0/completion_core/trait.CompletionFuture.html)相比，
它不是unsafe的。因为所有的约束都可以通过语言提供的原语来满足。


