use core::sync::atomic::{AtomicUsize, Ordering};
use alloc::vec;
use alloc::vec::Vec;

const RINGBUF_LENGTH: usize = 10_000;

pub struct RingBuffer<T: Clone + Sized> {
  buffer: Vec<Option<T>>,
  readidx: AtomicUsize,
  writeidx: AtomicUsize,
}

// assume that MPSC(Multiple Producer Single Consumer)
impl<T: Clone + Sized> RingBuffer<T> {
  pub fn new() -> RingBuffer<T> {
    RingBuffer {
      buffer: vec![None; RINGBUF_LENGTH],
      readidx: AtomicUsize::new(0),
      writeidx: AtomicUsize::new(0),
    }
  }

  pub fn get(&mut self) -> Option<&T> {
    loop {
      let wix = self.writeidx.load(Ordering::SeqCst);
      let rix = self.readidx.load(Ordering::SeqCst);
      let new_rix = (rix + 1) % RINGBUF_LENGTH;
      if rix == wix {
        return None;
      } else if rix == self.readidx.compare_and_swap(rix, new_rix, Ordering::SeqCst) {
        match &self.buffer[rix] {
          Some(itm) => return Some(&itm),
          None => return None,
        }
      } 
    }
  }

  pub fn put(&mut self, val: T) {
    let mut wix;
    let mut new_wix;
    loop {
      let rix = self.readidx.load(Ordering::SeqCst);
      wix = self.writeidx.load(Ordering::SeqCst);
      new_wix = (wix + 1) % RINGBUF_LENGTH;
      if rix != new_wix {
        if wix == self.writeidx.compare_and_swap(wix, new_wix, Ordering::SeqCst) {
          // got space and success to get it.
          break;
        }
      }
    }
    self.buffer[wix] = Some(val);
  }
}

impl<T: Default + Clone + Sized> Default for RingBuffer<T> {
  fn default() -> Self {
    RingBuffer { buffer: Default::default(), readidx: AtomicUsize::new(0), writeidx: AtomicUsize::new(0) }
  }
}

unsafe impl<T: Default + Clone + Sized> Sync for RingBuffer<T> {}
