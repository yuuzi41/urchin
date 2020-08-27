use core::future::Future;
use core::pin::Pin;
use core::time::Duration;
use core::task::{Context, Poll, Waker};

use core::sync::atomic::{ AtomicBool, AtomicU64, Ordering };

use alloc::sync::Arc;
use crate::spinlock::Spinlock;

use crossbeam_queue::{ArrayQueue, PushError};

lazy_static! {
  static ref REGISTERED_TIMER_STATES: ArrayQueue<Arc<SharedState>> = ArrayQueue::new(100_000);
}

struct SharedState {
  completed: AtomicBool,
  expired_monotonic: AtomicU64, 
  waker: Spinlock<Option<Waker>>,
}

pub struct TimerFuture {
  shared_state: Arc<SharedState>,
}

impl TimerFuture {
  pub fn new(duration: Duration) -> Self {
    //todo: use other clock
    use crate::arch::x86_64::kvmclock::get_monotonic_time;
    let nowtime = get_monotonic_time();
    let expiretime = nowtime + duration.as_nanos() as u64;

    let shared_state = Arc::new(
      SharedState {
        completed: AtomicBool::new(false),
        expired_monotonic: AtomicU64::new(expiretime),
        waker: Spinlock::new(None),
      }
    );

    // register the future
    REGISTERED_TIMER_STATES.push(Arc::clone(&shared_state)).expect("too many tasks queued");

    TimerFuture { shared_state }
  }
}

impl Future for TimerFuture {
  type Output = ();
  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    if self.shared_state.completed.load(Ordering::Relaxed) {
      Poll::Ready(())
    } else {
      let mut waker =  self.shared_state.waker.lock();
      *waker = Some(cx.waker().clone());
      Poll::Pending
    }
  }
}


pub fn check_timerfutures_from_interrupt() {
  //todo: use other clock
  use crate::arch::x86_64::kvmclock::get_monotonic_time;
  let nowtime = get_monotonic_time();

  //println!("DEBUG: nowtime={}", nowtime);

  for _i in 0..REGISTERED_TIMER_STATES.len() {
    if let Ok(shared_state) = REGISTERED_TIMER_STATES.pop() {
      //println!("DEBUG: future is got. expire={}", shared_state.expired_monotonic.load(Ordering::Relaxed));
      if nowtime > shared_state.expired_monotonic.load(Ordering::Relaxed) {
        //expired
        //println!("DEBUG: timerfuture has been expired.");
        shared_state.completed.store(true, Ordering::Relaxed);
        if let Some(waker) = shared_state.waker.lock().take() {
          //println!("DEBUG: timerfuture wake.");
          waker.wake()
        }
      } else {
        //not expired, so return it.
        REGISTERED_TIMER_STATES.push(shared_state).expect("too many tasks queued");
      }
    }
  }
}
