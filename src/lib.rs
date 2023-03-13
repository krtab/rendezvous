#![warn(clippy::undocumented_unsafe_blocks)]

//! Enables threads to synchronize the beginning or end of some computation.
//!
//! # Rendezvous vs barriers
//!
//! [`Rendezvous`] is very similar to [`Barrier`], but there
//! are a few differences:
//!
//! * [`Barrier`] needs to know the number of threads at construction, while
//!   [`Rendezvous`] is cloned to register more threads.
//!
//! * A [`Barrier`] can be reused even after all threads have synchronized,
//!   while a [`Rendezvous`] synchronizes threads only once.
//!
//! * All threads wait for others to reach the [`Barrier`]. With [`Rendezvous`],
//!   each thread can choose to either wait for other threads or to continue
//!   without blocking.
//!
//! * When a thread holding a [`Rendezvous`] panics, its copy of the
//!   [`Rendezvous`] is dropped and the other threads will not be blocked
//!   waiting for it.
//!
//! # Examples
//!
//! ```
//! use rendezvous::Rendezvous;
//! use std::thread;
//!
//! // Create a new Rendezvous.
//! let rdv = Rendezvous::new();
//!
//! for _ in 0..4 {
//!     // Create another reference to the rendezvous.
//!     let rdv = rdv.clone();
//!
//!     thread::spawn(move || {
//!         // Do some work.
//!
//!         // Drop the reference to the rendezvous.
//!         drop(rdv);
//!     });
//! }
//!
//! // Block until all threads have finished their work.
//! rdv.wait();
//! # std::thread::sleep(std::time::Duration::from_millis(500)); // wait for background threads closed: https://github.com/rust-lang/miri/issues/1371
//! ```
//!
//! # Other implementations
//!
//! There are many other implementations of the same construct, however, this is
//! -- to our knwoledge -- the only one relying on atomic and futexes. Below are
//!
//! - [`crossbeams`](https://docs.rs/crossbeam/0.8.2/crossbeam/sync/struct.WaitGroup.htm)
//!   offers the exact same functionnalities. This crate's documentations is
//!   adapted from crossbeam's MIT licensed one.
//! - [`adaptive_barrier`](https://docs.rs/adaptive-barrier/latest/adaptive_barrier)
//!   offers poisoning and leader election on top of the base functionnalities.
//!
//! [`Barrier`]: std::sync::Barrier
use std::{
    fmt::Debug,
    mem::forget,
    ptr::NonNull,
    sync::atomic::{AtomicU32, Ordering},
};

/// An adaptive barrier or waitgroup. See the [crate] documentation for more.
///
/// # Remarks
///
/// - There cannot be more than 2³² - 1 simultaneous copies of a single
///   rendezvous.
pub struct Rendezvous {
    ptr: NonNull<RDVInner>,
}

struct RDVInner {
    live: AtomicU32,
    alloc_dep: AtomicU32,
}

impl Rendezvous {
    /// Creates a new `Rendezvous`. Clone it so that other threads can
    /// synchronize on it.
    pub fn new() -> Self {
        let boxed = Box::new(RDVInner {
            live: AtomicU32::new(1),
            alloc_dep: AtomicU32::new(1),
        });
        Self {
            // SAFETY: Box::into_raw cannot be null.
            ptr: unsafe { NonNull::new_unchecked(Box::into_raw(boxed)) },
        }
    }

    /// Drops this reference and waits until all other references are dropped.
    pub fn wait(self) {
        let ptr = self.ptr;
        forget(self);
        // Scope-invariant:
        // inner.alloc_dep > 0
        // which implies that self.ptr is still valid
        {
            // Safety: Because of the scope invariant
            // the pointer will remain valid until the scope's end.
            let inner = unsafe { ptr.as_ref() };
            let mut l = inner.live.fetch_sub(1, Ordering::AcqRel) - 1;
            if l == 0 {
                // We were the last live barrier
                atomic_wait::wake_all(&inner.live);
            }
            while l > 0 {
                // There are still some live barriers
                atomic_wait::wait(&inner.live, l);
                l = inner.live.load(Ordering::Acquire);
            }
        }
        // Safety: the invariant from the scope above is still true
        // and is broken in this very instruction
        if unsafe { ptr.as_ref() }
            .alloc_dep
            .fetch_sub(1, Ordering::AcqRel)
            == 1
        {
            // Safety: we were the last alloc_dependent barrier so nobody else
            // is trying to drop the inner and we can do it.
            unsafe { Box::from_raw(ptr.as_ptr()) };
        }
    }
}

impl Drop for Rendezvous {
    fn drop(&mut self) {
        // Scope-invariant:
        // inner.alloc_dep > 0
        // which implies that self.ptr is still valid
        {
            // Safety: Because of the scope invariant
            // the pointer will remain valid until the scope's end.
            let inner = unsafe { self.ptr.as_ref() };
            if inner.live.fetch_sub(1, Ordering::AcqRel) == 1 {
                //TODO(arthur): maybe do only if there are waiting threads
                atomic_wait::wake_all(&inner.live);
            }
        }
        // Safety: the invariant from the scope above is still true
        // and is broken in this very instruction
        if unsafe { self.ptr.as_ref() }
            .alloc_dep
            .fetch_sub(1, Ordering::AcqRel)
            == 1
        {
            // Safety: we were the last alloc_dependent barrier so nobody else
            // is trying to drop the inner and we can do it.
            unsafe { Box::from_raw(self.ptr.as_ptr()) };
        }
    }
}

impl Clone for Rendezvous {
    fn clone(&self) -> Self {
        // Safety: self exist so the ptr is valid
        let inner = unsafe { self.ptr.as_ref() };
        inner
            .alloc_dep
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |n| n.checked_add(1))
            .expect("There should not be more than 2^32 - 1 clones of one Rendezvous.");
        // This one cannot overflow because live < alloc_dep
        // at all times
        inner.live.fetch_add(1, Ordering::Acquire);
        Self {
            ptr: self.ptr,
        }
    }
}

// Marker traits implementations

// Safety: it is send by design.
unsafe impl Send for Rendezvous {}
// Safety: this is also sync:
// all methods taking self by reference (only clone for now) only use it as a
// smart pointer and do not change the allocation.
unsafe impl Sync for Rendezvous {}

// Common traits implementations

impl Default for Rendezvous {
    fn default() -> Self {
        Self::new()
    }
}

impl Debug for Rendezvous {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Safety: self exist so the ptr is valid
        let inner = unsafe { self.ptr.as_ref() };
        f.debug_struct("Rendezvous")
            .field("live barriers", &inner.live.load(Ordering::Acquire))
            .field(
                "total allocations (live + waiting)",
                &inner.alloc_dep.load(Ordering::Acquire),
            )
            .finish()
    }
}
