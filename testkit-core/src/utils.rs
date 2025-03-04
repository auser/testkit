use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Macro to automatically box an async block for use with the boxed database API
///
/// This macro makes the boxed database API more ergonomic by hiding the need to
/// manually use Box::pin() around async blocks.
#[macro_export]
macro_rules! boxed_async {
    // Match an async block with move
    (async move $block:block) => {
        Box::pin(async move $block)
    };
    // Match a regular async block
    (async $block:block) => {
        Box::pin(async $block)
    };
}

/// Boxes an async closure to handle lifetime issues
///
/// This function takes a closure that returns a Future and wraps it in a
/// Pin<Box<dyn Future>> to solve lifetime problems. This is particularly useful
/// for async closures that capture variables from their environment.
pub fn boxed_future<T, F, Fut, E>(
    f: F,
) -> impl FnOnce(T) -> Pin<Box<dyn Future<Output = Result<(), E>> + Send>>
where
    F: FnOnce(T) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), E>> + Send + 'static,
    T: Send + 'static,
    E: Send + 'static,
{
    move |t| {
        let future = f(t);
        Box::pin(future) as Pin<Box<dyn Future<Output = Result<(), E>> + Send>>
    }
}

/// Boxes an async closure that returns a Result type
///
/// This function takes a closure that returns a Future with a Result and wraps it in a
/// Pin<Box<dyn Future>> to solve lifetime problems. This is particularly useful
/// for async closures that capture variables from their environment.
pub fn boxed_future_with_result<T, F, Fut, R, E>(
    f: F,
) -> impl FnOnce(T) -> Pin<Box<dyn Future<Output = Result<R, E>> + Send>>
where
    F: FnOnce(T) -> Fut + Send + 'static,
    Fut: Future<Output = Result<R, E>> + Send + 'static,
    T: Send + 'static,
    R: Send + 'static,
    E: Send + 'static,
{
    move |t| {
        let future = f(t);
        Box::pin(future) as Pin<Box<dyn Future<Output = Result<R, E>> + Send>>
    }
}

/// A utility struct to enable auto-boxing of futures with different lifetimes
pub struct AutoBoxFuture<'a, F, Fut, T, R>
where
    F: FnOnce(&'a mut T) -> Fut + 'static,
    Fut: Future<Output = R> + 'static,
    T: 'a,
    R: 'static,
{
    #[allow(dead_code)]
    f: Option<F>,
    arg: PhantomData<&'a mut T>,
    #[allow(dead_code)]
    fut: Option<Fut>,
    _phantom: PhantomData<R>,
}

impl<'a, F, Fut, T, R> AutoBoxFuture<'a, F, Fut, T, R>
where
    F: FnOnce(&'a mut T) -> Fut + 'static,
    Fut: Future<Output = R> + 'static,
    T: 'a,
    R: 'static,
{
    /// Create a new auto-boxing future
    pub fn new(f: F) -> Self {
        AutoBoxFuture {
            f: Some(f),
            arg: PhantomData,
            fut: None,
            _phantom: PhantomData,
        }
    }
}

impl<'a, F, Fut, T, R> Future for AutoBoxFuture<'a, F, Fut, T, R>
where
    F: FnOnce(&'a mut T) -> Fut + 'static,
    Fut: Future<Output = R> + 'static,
    T: 'a,
    R: 'static,
{
    type Output = R;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Since T is bound by 'a, we can't actually call F here without having a &'a mut T
        // This AutoBoxFuture is just a placeholder for the compiler - the real implementation
        // will use a different approach.
        panic!("AutoBoxFuture should never be polled directly");
    }
}

/// Helper function to auto-box a future closure that captures variables
/// and convert it to a Pin<Box<dyn Future>> with the correct lifetime
pub fn auto_box_future<T, F, Fut, R>(
    f: F,
) -> impl for<'a> FnOnce(&'a mut T) -> Pin<Box<dyn Future<Output = R> + Send + 'a>>
where
    for<'a> F: FnOnce(&'a mut T) -> Fut + Send + Sync + 'static,
    for<'a> Fut: Future<Output = R> + Send + 'a,
    R: Send + 'static,
{
    move |arg| Box::pin(f(arg))
}
