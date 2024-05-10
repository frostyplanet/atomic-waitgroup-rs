//!
//! A waitgroup support async with advanced features,
//! implemented with atomic operations to reduce locking in mind.
//!
//! # Features
//!
//! * wait_to() is supported to wait for a value larger than zero.
//!
//! * wait() & wait_to() can be canceled by tokio::time::timeout or futures::select!.
//!
//! * Assumes only one thread calls wait(). If multiple concurrent wait() is detected,
//! will panic for this invalid usage.
//!
//! * done() can be called by multiple coroutines other than the one calls wait().
//!
//! # Example
//!
//! ```
//! extern crate atomic_waitgroup;
//! use atomic_waitgroup::WaitGroup;
//! use tokio::runtime::Runtime;
//!
//! let rt = Runtime::new().unwrap();
//! let wg = WaitGroup::new();
//! rt.block_on(async move {
//!     for i in 0..2 {
//!         let _guard = wg.add_guard();
//!         tokio::spawn(async move {
//!            // Do something
//!             drop(_guard);
//!         });
//!     }
//!     match tokio::time::timeout(
//!         tokio::time::Duration::from_secs(1),
//!         wg.wait_to(1)).await {
//!         Ok(_) => {
//!             assert!(wg.left() <= 1);
//!         }
//!         Err(_) => {
//!             println!("wg.wait_to(1) timeouted");
//!         }
//!     }
//! });
//!
//!

use log::error;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicI64, AtomicU64, Ordering},
        Arc,
    },
    task::{Context, Poll, Waker},
};

use parking_lot::Mutex;

/*

NOTE: Multiple atomic operation must happen at the same order

WaitGroupFuture |   done()
----------
left.load()     |   left -=1
waiting = true  |   load_waiting
left.load ()    |
------------

*/
pub struct WaitGroup(Arc<WaitGroupInner>);

// do not allow multiple wait
impl Clone for WaitGroup {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl WaitGroup {
    pub fn new() -> Self {
        Self(WaitGroupInner::new())
    }

    /// Return the count left inside this WaitGroup
    #[inline(always)]
    pub fn left(&self) -> usize {
        let count = self.0.left.load(Ordering::SeqCst);
        if count < 0 {
            error!("WaitGroup.left {} < 0", count);
            panic!("WaitGroup.left {} < 0", count);
        }
        count as usize
    }

    /// Add specified count.
    ///
    /// NOTE: although add() does not conflict with wait, we advise to avoid concurrency which lead
    /// to ill logic
    #[inline(always)]
    pub fn add(&self, i: usize) {
        self.0.left.fetch_add(i as i64, Ordering::SeqCst);
    }

    /// Add one to the WaitGroup, return a guard to decrease the count on drop.
    ///
    /// # Example
    ///
    /// ```
    /// extern crate atomic_waitgroup;
    /// use atomic_waitgroup::WaitGroup;
    /// use tokio::runtime::Runtime;
    ///
    /// let wg = WaitGroup::new();
    /// let rt = Runtime::new().unwrap();

    /// rt.block_on(async move {
    ///     let _guard = wg.add_guard();
    ///     tokio::spawn(async move {
    ///         // Do something
    ///         drop(_guard);
    ///     });
    ///     wg.wait().await;
    /// });
    #[inline(always)]
    pub fn add_guard(&self) -> WaitGroupGuard {
        self.0.left.fetch_add(1, Ordering::SeqCst);
        WaitGroupGuard {
            inner: self.0.clone(),
        }
    }

    /// Wait until specified count is left in the WaitGroup.
    ///
    /// return false means there's no waiting happened.
    ///
    /// return true means the blocking actually happened.
    ///
    /// # NOTE:
    ///
    /// * Only assume one waiting future at the same time, otherwise will panic.
    ///
    /// * Canceling future is supported.
    pub async fn wait_to(&self, target: usize) -> bool {
        let _self = self.0.as_ref();
        let left = _self.left.load(Ordering::Acquire);
        if left <= target as i64 {
            return false;
        }
        WaitGroupFuture {
            wg: &_self,
            target,
            waker_id: 0,
        }
        .await;
        return true;
    }

    /// Wait until zero count in the WaitGroup.
    ///
    /// # NOTE:
    ///
    /// * Only assume one waiting future at the same time, otherwise will panic.
    ///
    /// * Canceling future is supported.
    #[inline(always)]
    pub async fn wait(&self) {
        self.wait_to(0).await;
    }

    /// Decrease count by one.
    #[inline]
    pub fn done(&self) {
        let inner = self.0.as_ref();
        inner.done(1);
    }

    /// Decrease count by specified value
    #[inline]
    pub fn done_many(&self, count: usize) {
        let inner = self.0.as_ref();
        inner.done(count as i64);
    }
}

pub struct WaitGroupGuard {
    inner: Arc<WaitGroupInner>,
}

impl Drop for WaitGroupGuard {
    fn drop(&mut self) {
        let inner = &self.inner;
        inner.done(1);
    }
}

struct WaitGroupInner {
    left: AtomicI64,
    waiting: AtomicI64,
    waker: Mutex<Option<Waker>>,
    waker_id: AtomicU64,
}

impl WaitGroupInner {
    #[inline(always)]
    fn new() -> Arc<Self> {
        Arc::new(Self {
            left: AtomicI64::new(0),
            waiting: AtomicI64::new(-1),
            waker: Mutex::new(None),
            waker_id: AtomicU64::new(0),
        })
    }
    #[inline]
    fn done(&self, count: i64) {
        let left = self.left.fetch_sub(count, Ordering::SeqCst) - count;
        let waiting = self.waiting.load(Ordering::Acquire);
        if left < 0 {
            error!("WaitGroup.left {} < 0", left);
            panic!("WaitGroup.left {} < 0", left);
        }
        if waiting < 0 {
            return;
        }
        if left <= waiting {
            // Do not take waker, it may be false waken when done() happened before newer wait()
            if let Some(waker) = self.waker.lock().as_ref() {
                waker.wake_by_ref();
            }
        }
    }

    /// Once waker set, waker might be false waken many times
    /// Returns: waker_id
    #[inline]
    fn set_waker(&self, waker: Waker, target: usize) -> u64 {
        let waker_id = self.waker_id.fetch_add(1, Ordering::SeqCst) + 1;
        {
            let mut guard = self.waker.lock();
            guard.replace(waker);
            let old_target = self.waiting.swap(target as i64, Ordering::SeqCst);
            if old_target >= 0 {
                panic!("Concurrent wait() by multiple coroutines is not supported")
            }
        }
        waker_id
    }

    #[inline]
    fn cancel_wait(&self, waker_id: u64) {
        let mut guard = self.waker.lock();
        // In case wait() is canceled, eg. tokio timeout, do not disrupt other thread wait()
        if self.waker_id.load(Ordering::Acquire) == waker_id {
            self.waiting.store(-1, Ordering::Release);
            let _ = guard.take();
        }
    }
}

struct WaitGroupFuture<'a> {
    wg: &'a WaitGroupInner,
    target: usize,
    waker_id: u64,
}

impl<'a> WaitGroupFuture<'a> {
    #[inline(always)]
    fn _poll(&mut self) -> bool {
        let cur = self.wg.left.load(Ordering::Acquire);
        if cur <= self.target as i64 {
            self._clear();
            true
        } else {
            false
        }
    }

    #[inline(always)]
    fn _clear(&mut self) {
        if self.waker_id == 0 {
            return;
        }
        self.wg.cancel_wait(self.waker_id);
        self.waker_id = 0;
    }
}

/// When wait() is canceled with timeout(),  make sure it clear the waker.
impl<'a> Drop for WaitGroupFuture<'a> {
    fn drop(&mut self) {
        self._clear();
    }
}

impl<'a> Future for WaitGroupFuture<'a> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let _self = self.get_mut();
        if _self.waker_id == 0 {
            if _self._poll() {
                return Poll::Ready(());
            }
            _self.waker_id = _self.wg.set_waker(ctx.waker().clone(), _self.target);
        }
        if _self._poll() {
            return Poll::Ready(());
        }
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    extern crate rand;

    use std::time::Duration;
    use tokio::time::{sleep, timeout};

    use super::*;

    fn make_runtime(threads: usize) -> tokio::runtime::Runtime {
        return tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(threads)
            .build()
            .unwrap();
    }

    #[test]
    fn test_inner() {
        make_runtime(1).block_on(async move {
            let wg = WaitGroup::new();
            wg.add(2);
            let _wg = wg.clone();
            let th = tokio::spawn(async move {
                assert!(_wg.wait_to(1).await);
            });
            sleep(Duration::from_secs(1)).await;
            assert_eq!(wg.0.waker_id.load(Ordering::Acquire), 1);
            {
                let guard = wg.0.waker.lock();
                assert!(guard.is_some());
                assert_eq!(wg.0.waiting.load(Ordering::Acquire), 1);
            }
            wg.done();
            let _ = th.await;
            assert_eq!(wg.0.waker_id.load(Ordering::Acquire), 1);
            assert_eq!(wg.0.waiting.load(Ordering::Acquire), -1);
            assert_eq!(wg.left(), 1);
            wg.done();
            assert_eq!(wg.left(), 0);
            assert_eq!(wg.wait_to(0).await, false);
        });
    }

    #[test]
    fn test_cancel() {
        let wg = WaitGroup::new();
        make_runtime(1).block_on(async move {
            wg.add(1);
            println!("test timeout");
            assert!(timeout(Duration::from_secs(1), wg.wait()).await.is_err());
            println!("timeout happened");
            assert_eq!(wg.0.waiting.load(Ordering::Acquire), -1);
            wg.done();
            wg.add(2);
            wg.done_many(2);
            wg.add(2);
            let _wg = wg.clone();
            let th = tokio::spawn(async move {
                _wg.wait().await;
            });
            sleep(Duration::from_millis(200)).await;
            assert_eq!(wg.0.waker_id.load(Ordering::Acquire), 2);
            wg.done();
            wg.done();
            let _ = th.await;
        });
    }
}
