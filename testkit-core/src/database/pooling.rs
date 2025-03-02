// Generic resource pooling system for database testkit
use std::{
    future::Future,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::Arc,
};

use parking_lot::Mutex;

// Type aliases for clarity
type Stack<T> = Vec<T>;
type Init<T> =
    Box<dyn Fn() -> Pin<Box<dyn Future<Output = T> + Send + 'static>> + Send + Sync + 'static>;
type Reset<T> =
    Box<dyn Fn(T) -> Pin<Box<dyn Future<Output = T> + Send + 'static>> + Send + Sync + 'static>;

/// Generic object pool for any reusable resource
pub struct ResourcePool<T> {
    resources: Arc<Mutex<Stack<T>>>,
    init: Arc<Init<T>>,
    reset: Arc<Reset<T>>,
}

impl<T> ResourcePool<T> {
    /// Create a new resource pool with initialization and reset functions
    pub fn new(init: Init<T>, reset: Reset<T>) -> Self {
        Self {
            resources: Arc::new(Mutex::new(Stack::new())),
            init: Arc::new(init),
            reset: Arc::new(reset),
        }
    }

    /// Get a resource from the pool, either by reusing an existing one
    /// or creating a new one if none are available
    pub async fn acquire(&self) -> Reusable<T> {
        let resource = self.resources.lock().pop();
        let resource = if let Some(resource) = resource {
            (self.reset)(resource).await
        } else {
            (self.init)().await
        };
        Reusable::new(self, resource)
    }

    /// Return a resource to the pool for future reuse
    pub fn release(&self, t: T) {
        self.resources.lock().push(t);
    }

    /// Create a shared pool that uses the same resource stack
    pub fn shared(&self) -> Arc<Self> {
        Arc::new(Self {
            resources: self.resources.clone(),
            init: self.init.clone(),
            reset: self.reset.clone(),
        })
    }
}

/// Wrapper for a reusable resource that returns it to the pool when dropped
pub struct Reusable<T> {
    pool: Arc<ResourcePool<T>>,
    data: Option<T>,
}

impl<T> Reusable<T> {
    fn new(pool: &ResourcePool<T>, t: T) -> Self {
        Self {
            pool: Arc::new(ResourcePool {
                resources: pool.resources.clone(),
                init: pool.init.clone(),
                reset: pool.reset.clone(),
            }),
            data: Some(t),
        }
    }

    /// Explicitly release the resource back to the pool
    pub fn release(mut self) {
        if let Some(data) = self.data.take() {
            self.pool.release(data);
        }
    }
}

const DATA_MUST_CONTAIN_SOME: &str = "data must always contain a [Some] value";

impl<T> Deref for Reusable<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.data.as_ref().expect(DATA_MUST_CONTAIN_SOME)
    }
}

impl<T> DerefMut for Reusable<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data.as_mut().expect(DATA_MUST_CONTAIN_SOME)
    }
}

impl<T> Drop for Reusable<T> {
    #[inline]
    fn drop(&mut self) {
        if let Some(data) = self.data.take() {
            self.pool.release(data);
        }
    }
}
