use std::{
    future::Future,
    ops::{Deref, DerefMut},
    pin::Pin,
};

use parking_lot::Mutex;

type Stack<T> = Vec<T>;
type Init<T> =
    Box<dyn Fn() -> Pin<Box<dyn Future<Output = T> + Send + 'static>> + Send + Sync + 'static>;
type Reset<T> =
    Box<dyn Fn(T) -> Pin<Box<dyn Future<Output = T> + Send + 'static>> + Send + Sync + 'static>;

#[allow(dead_code)]
pub(crate) struct ObjectPool<T> {
    objects: Mutex<Stack<T>>,
    init: Init<T>,
    reset: Reset<T>,
}

#[allow(dead_code)]
impl<T> ObjectPool<T> {
    pub(crate) fn new(init: Init<T>, reset: Reset<T>) -> Self {
        Self {
            objects: Mutex::new(Stack::new()),
            init,
            reset,
        }
    }

    pub(crate) async fn pull(&self) -> Reusable<T> {
        let object = self.objects.lock().pop();
        let object = if let Some(object) = object {
            (self.reset)(object).await
        } else {
            (self.init)().await
        };
        Reusable::new(self, object)
    }

    fn attach(&self, t: T) {
        self.objects.lock().push(t);
    }
}

/// Reusable object wrapper
pub struct Reusable<'a, T> {
    pool: &'a ObjectPool<T>,
    data: Option<T>,
}

#[allow(dead_code)]
impl<'a, T> Reusable<'a, T> {
    fn new(pool: &'a ObjectPool<T>, t: T) -> Self {
        Self {
            pool,
            data: Some(t),
        }
    }
}

const DATA_MUST_CONTAIN_SOME: &str = "data must always contain a [Some] value";

impl<'a, T> Deref for Reusable<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.data.as_ref().expect(DATA_MUST_CONTAIN_SOME)
    }
}

impl<'a, T> DerefMut for Reusable<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data.as_mut().expect(DATA_MUST_CONTAIN_SOME)
    }
}

impl<'a, T> Drop for Reusable<'a, T> {
    #[inline]
    fn drop(&mut self) {
        self.pool
            .attach(self.data.take().expect(DATA_MUST_CONTAIN_SOME));
    }
}
