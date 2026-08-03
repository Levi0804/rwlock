#[cfg(feature = "loom")]
pub(crate) use {
    loom::cell::UnsafeCell,
    loom::hint::spin_loop,
    loom::sync::atomic::AtomicIsize,
    loom::sync::atomic::Ordering::{Acquire, Relaxed, Release},
    loom::sync::atomic::fence,
};

#[cfg(not(feature = "loom"))]
pub(crate) use {
    std::cell::UnsafeCell,
    std::hint::spin_loop,
    std::sync::atomic::AtomicIsize,
    std::sync::atomic::Ordering::{Acquire, Relaxed, Release},
    std::sync::atomic::fence,
};
