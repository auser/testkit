use std::future::Future;
use std::pin::Pin;

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
    move |t| Box::pin(f(t))
}

/// Similar to boxed_future but allows returning a value
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
    move |t| Box::pin(f(t))
}
