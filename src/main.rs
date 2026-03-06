use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::AtomicIsize;
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};

pub struct Rw<T> {
    data: UnsafeCell<T>,
    counter: AtomicIsize,
}

unsafe impl<T> Send for Rw<T> where T: Send {}
unsafe impl<T> Sync for Rw<T> where T: Send + Sync {}

impl<T> Rw<T> {
    pub fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
            counter: AtomicIsize::new(0),
        }
    }

    pub fn read<'read>(&'read self) -> RwReadGuard<'read, T> {
        let mut current = self.counter.load(Relaxed);
        loop {
            match self
                .counter
                .compare_exchange_weak(current, current + 1, Acquire, Relaxed)
            {
                Ok(_) => break,
                Err(_) => {
                    while self.counter.load(Relaxed) < 0 {
                        std::hint::spin_loop();
                    }
                    current = self.counter.load(Relaxed);
                }
            }
        }

        RwReadGuard { rw: self }
    }

    pub fn write<'write>(&'write self) -> RwWriteGuard<'write, T> {
        while self
            .counter
            .compare_exchange_weak(0, -1, Acquire, Relaxed)
            .is_err()
        {
            std::hint::spin_loop();
        }

        RwWriteGuard { rw: self }
    }
}

pub struct RwReadGuard<'read, T> {
    rw: &'read Rw<T>,
}

impl<T> Deref for RwReadGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.rw.data.get() }
    }
}

impl<T> Drop for RwReadGuard<'_, T> {
    fn drop(&mut self) {
        let mut current = self.rw.counter.load(Relaxed);
        loop {
            match self
                .rw
                .counter
                .compare_exchange_weak(current, current - 1, Release, Relaxed)
            {
                Ok(_) => break,
                Err(new) => current = new,
            }
        }
    }
}

pub struct RwWriteGuard<'write, T> {
    rw: &'write Rw<T>,
}

impl<T> Deref for RwWriteGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.rw.data.get() }
    }
}

impl<T> DerefMut for RwWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.rw.data.get() }
    }
}

impl<T> Drop for RwWriteGuard<'_, T> {
    fn drop(&mut self) {
        self.rw.counter.store(0, Release);
    }
}

fn main() {
    let rw: &'static _ = Box::leak(Box::new(Rw::new(0)));

    std::thread::scope(|s| {
        for _ in 0..10000 {
            s.spawn(|| {
                let _r = rw.read();
            });
        }
        for _ in 0..10000 {
            s.spawn(|| {
                let mut w = rw.write();
                *w += 1;
            });
        }
    });

    assert_eq!(*rw.read(), 10000);
}
