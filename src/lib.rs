use std::ops::{Deref, DerefMut};

mod sync;
use crate::sync::{Acquire, AtomicIsize, Relaxed, Release, UnsafeCell, spin_loop};

pub struct Rw<T> {
    data: UnsafeCell<T>,
    counter: AtomicIsize,
}

// TODO
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
            if current < 0 {
                current = self.counter.load(Relaxed);
            } else {
                match self
                    .counter
                    .compare_exchange_weak(current, current + 1, Acquire, Relaxed)
                {
                    Ok(_) => break,
                    Err(next_prev) => {
                        current = next_prev;
                    }
                }
            }
            spin_loop();
        }

        RwReadGuard { rw: self }
    }

    pub fn write<'write>(&'write self) -> RwWriteGuard<'write, T> {
        while self
            .counter
            .compare_exchange_weak(0, -1, Acquire, Relaxed)
            .is_err()
        {
            spin_loop();
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
        #[cfg(not(feature = "loom"))]
        unsafe {
            &*self.rw.data.get()
        }
        #[cfg(feature = "loom")]
        unsafe {
            &*self.rw.data.with_mut(|p| p)
        }
    }
}

impl<T> Drop for RwReadGuard<'_, T> {
    fn drop(&mut self) {
        let mut current = self.rw.counter.load(Relaxed);
        loop {
            if current < 0 {
                current = self.rw.counter.load(Relaxed);
            } else {
                match self
                    .rw
                    .counter
                    .compare_exchange_weak(current, current - 1, Relaxed, Relaxed)
                {
                    Ok(_) => break,
                    Err(new) => current = new,
                }
            }
        }
    }
}

// TODO:
unsafe impl<T> Send for RwReadGuard<'_, T> {}
unsafe impl<T> Sync for RwReadGuard<'_, T> {}

pub struct RwWriteGuard<'write, T> {
    rw: &'write Rw<T>,
}

impl<T> Deref for RwWriteGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        #[cfg(not(feature = "loom"))]
        unsafe {
            &*self.rw.data.get()
        }
        #[cfg(feature = "loom")]
        unsafe {
            &*self.rw.data.with_mut(|p| p)
        }
    }
}

impl<T> DerefMut for RwWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        #[cfg(not(feature = "loom"))]
        unsafe {
            &mut *self.rw.data.get()
        }
        #[cfg(feature = "loom")]
        unsafe {
            &mut *self.rw.data.with_mut(|p| p)
        }
    }
}

impl<T> Drop for RwWriteGuard<'_, T> {
    fn drop(&mut self) {
        self.rw.counter.store(0, Release);
    }
}

// TODO:
unsafe impl<T> Send for RwWriteGuard<'_, T> {}
unsafe impl<T> Sync for RwWriteGuard<'_, T> {}

#[cfg(test)]
#[cfg(feature = "loom")]
mod tests {
    use super::*;

    #[test]
    fn test_rw_lock_read_write() {
        loom::model(|| {
            let rw: &'static _ = Box::leak(Box::new(Rw::new(0)));

            let handle1 = loom::thread::spawn(|| {
                let _r = rw.read();
            });
            let handle2 = loom::thread::spawn(|| {
                let mut w = rw.write();
                *w += 1;
            });

            handle1.join().unwrap();
            handle2.join().unwrap();

            assert_eq!(*rw.read(), 1);
        });
    }

    #[test]
    fn test_rw_lock_read_read() {
        loom::model(|| {
            let rw: &'static _ = Box::leak(Box::new(Rw::new(0)));

            let handle1 = loom::thread::spawn(|| {
                let _r = rw.read();
            });
            let handle2 = loom::thread::spawn(|| {
                let _r = rw.read();
            });

            handle1.join().unwrap();
            handle2.join().unwrap();

            assert_eq!(*rw.read(), 0);
        });
    }

    #[test]
    fn test_rw_lock_write_write() {
        loom::model(|| {
            let rw: &'static _ = Box::leak(Box::new(Rw::new(0)));

            let handle1 = loom::thread::spawn(|| {
                let mut w = rw.write();
                *w += 1;
            });
            let handle2 = loom::thread::spawn(|| {
                let mut w = rw.write();
                *w += 1;
            });

            handle1.join().unwrap();
            handle2.join().unwrap();

            assert_eq!(*rw.read(), 2);
        });
    }
}

#[test]
#[cfg(not(feature = "loom"))]
fn test_concurr_rw_lock_fuzzing() {
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
