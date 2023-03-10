use std::{
    marker::PhantomData,
    ptr::NonNull,
    sync::atomic::{AtomicU32, Ordering}, mem::forget,
};

pub struct Rendezvous {
    ptr: NonNull<RDVInner>,
    phantom: PhantomData<RDVInner>,
}

impl Rendezvous {
    pub fn new() -> Self {
        let boxed = Box::new(RDVInner {
            live: AtomicU32::new(1),
            waiting: AtomicU32::new(0),
        });
        Self {
            // SAFETY: Box::into_raw cannot be null.
            ptr: unsafe { NonNull::new_unchecked(Box::into_raw(boxed)) },
            phantom: PhantomData,
        }
    }

    pub fn wait(self) -> bool {
        let ptr = self.ptr;
        forget(self);
        {
            let inner = unsafe { ptr.as_ref() };
            inner.waiting.fetch_add(1, Ordering::Release);
            let mut l = inner.live.fetch_sub(1, Ordering::AcqRel) - 1;
            if l == 0 {
                atomic_wait::wake_all(&inner.live);
            }
            while l > 0 {
                atomic_wait::wait(&inner.live, l);
                l = inner.live.load(Ordering::Acquire);
            }
            if inner.waiting.fetch_sub(1, Ordering::AcqRel) > 0 {
                return false;
            }
        }
        unsafe { Box::from_raw(ptr.as_ptr()) };
        true
    }
}

impl Drop for Rendezvous {
    fn drop(&mut self) {
        {
            let inner = unsafe { self.ptr.as_ref() };
            if inner.live.fetch_sub(1, Ordering::AcqRel) > 1 {
                return;
            }
            if inner.waiting.load(Ordering::Acquire) > 0 {
                atomic_wait::wake_all(&inner.live);
                return;
            }
        }
        unsafe { Box::from_raw(self.ptr.as_ptr()) };
    }
}

impl Clone for Rendezvous {
    fn clone(&self) -> Self {
        unsafe { self.ptr.as_ref() }
            .live
            .fetch_add(1, Ordering::Acquire);
        Self {
            ptr: self.ptr.clone(),
            phantom: self.phantom.clone(),
        }
    }
}

struct RDVInner {
    live: AtomicU32,
    waiting: AtomicU32,
}
