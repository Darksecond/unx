use bootinfo::boot_info::MemoryMap;
use spinning_top::{Spinlock, SpinlockGuard, const_spinlock};

mod phys;
mod mapper;
mod heap;

pub struct Locked<A> {
    inner: Spinlock<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: const_spinlock(inner),
        }
    }

    pub fn lock(&self) -> SpinlockGuard<A> {
        self.inner.lock()
    }
}

pub fn init(map: &MemoryMap) {
    phys::init(map);
    mapper::init();
    heap::init();
}