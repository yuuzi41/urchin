use lock_api::{RawMutex, Mutex, MutexGuard, GuardSend};
use core::sync::atomic::{AtomicBool, Ordering};

use crate::interrupt;

// 1. Define our raw lock type
pub struct RawSpinlock(AtomicBool);

// 2. Implement RawMutex for this type
unsafe impl RawMutex for RawSpinlock {
    const INIT: RawSpinlock = RawSpinlock(AtomicBool::new(false));

    // A spinlock guard can be sent to another thread and unlocked there
    type GuardMarker = GuardSend;

    fn lock(&self) {
        // Note: This isn't the best way of implementing a spinlock, but it
        // suffices for the sake of this example.
        while !self.try_lock() {}
    }

    fn try_lock(&self) -> bool {
        //interrupt::disable_interrupt(); //interrupt is disabled while locked
        let is_locked = self.0
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok();
        if !is_locked {
            //interrupt::enable_interrupt(); //interrupt is disabled while locked
        }
        is_locked
    }

    unsafe fn unlock(&self) {
        self.0.store(false, Ordering::Release);
        //interrupt::enable_interrupt();
    }
}

// 3. Export the wrappers. This are the types that your users will actually use.
pub type Spinlock<T> = Mutex<RawSpinlock, T>;
pub type SpinlockGuard<'a, T> = MutexGuard<'a, RawSpinlock, T>;

pub const fn const_spinlock<T>(val: T) -> Spinlock<T> {
    Spinlock::const_new(RawSpinlock::INIT, val)
}