#![warn(clippy::undocumented_unsafe_blocks)]

use std::{
    marker::PhantomData,
    mem::forget,
    ptr::NonNull,
    sync::atomic::{AtomicU32, Ordering},
};

pub struct Rendezvous {
    ptr: NonNull<RDVInner>,
    phantom: PhantomData<RDVInner>,
}

unsafe impl Send for Rendezvous {}
// unsafe impl Sync for Rendezvous {}

impl Rendezvous {
    pub fn new() -> Self {
        let boxed = Box::new(RDVInner {
            live: AtomicU32::new(1),
            alloc_dep: AtomicU32::new(1),
        });
        Self {
            // SAFETY: Box::into_raw cannot be null.
            ptr: unsafe { NonNull::new_unchecked(Box::into_raw(boxed)) },
            phantom: PhantomData,
        }
    }

    pub fn wait(self) {
        let ptr = self.ptr;
        forget(self);
        // Scope-invariant:
        // inner.alloc_dep > 0
        // which implies that self.ptr is still valid
        {
            let inner = unsafe { ptr.as_ref() };
            // Safety: Because of the scope invariant
            // the pointer will remain valid until the scope's end.
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

impl Default for Rendezvous {
    fn default() -> Self {
        Self::new()
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
        inner.alloc_dep.fetch_add(1, Ordering::Acquire);
        // This one cannot overflow because live < alloc_dep
        // at all times
        inner.live.fetch_add(1, Ordering::Acquire);
        Self {
            ptr: self.ptr,
            phantom: self.phantom,
        }
    }
}

struct RDVInner {
    live: AtomicU32,
    alloc_dep: AtomicU32,
}
