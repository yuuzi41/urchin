use core::future::Future;
use core::task::{Context, Poll};
use core::time::Duration;

use alloc::sync::Arc;

use futures::future::{FutureExt, BoxFuture};
use futures::task::{ArcWake, waker_ref};

use crossbeam_queue::{ArrayQueue, PushError};

use crate::spinlock::*;

pub struct Executor {
  /// Spawned tasks
  tasks: Arc<ArrayQueue<Arc<Task>>>,
}

struct Task {
  future: Spinlock<Option<BoxFuture<'static, ()>>>,
  tasks: Arc<ArrayQueue<Arc<Task>>>,
}

impl ArcWake for Task {
  fn wake_by_ref(arc_self: &Arc<Self>) {
      // Implement `wake` by sending this task back onto the task channel
      // so that it will be polled again by the executor.
      let cloned = arc_self.clone();
      arc_self.tasks.push(cloned).expect("too many tasks queued");
  }
}

impl Executor {
  pub fn new() -> Self {
    Self {
      tasks: Arc::new(ArrayQueue::new(100_000)),
    }
  }

  /*
  pub fn block_on<T>(&self, future: impl Future<Output = ()> + 'static + Send) {

  }*/

  pub fn run(&self) {
    while let Ok(task) = self.tasks.pop() {
      let mut future_slot = task.future.lock();
      if let Some(mut future) = future_slot.take() {
        let waker = waker_ref(&task);
        let context = &mut Context::from_waker(&*waker);

        if let Poll::Pending = future.as_mut().poll(context) {
          *future_slot = Some(future);
        }
      }
    }
  }

  pub fn spawn(&self, future: impl Future<Output = ()> + 'static + Send) {
    let future = future.boxed();
    let task = Arc::new(Task {
      future: Spinlock::new(Some(future)),
      tasks: Arc::clone(&self.tasks), 
    });
    self.tasks.push(task).expect("too many tasks queued");
  }
}

