
use alloc::alloc::{alloc, dealloc, Layout};
use core::slice::{from_raw_parts, from_raw_parts_mut};
use core::sync::atomic::{Ordering, AtomicUsize};

pub enum Error {
  LayoutError,
}
  
pub struct Buffer {
  buffer_ptr: *mut u8,
  size: usize,
  position: AtomicUsize,
  layout: Layout,
}
  
impl Buffer {
  pub fn new(size: usize, align: usize) -> Result<Buffer, Error> {
    let actual_size = (((size-1) / align) + 1) * align;

    let layout = match Layout::from_size_align(actual_size, align) {
      Ok(l) => l,
      Err(e) => return Err(Error::LayoutError),
    };
    let ptr = unsafe {
      alloc(layout)
    };

    Ok(Buffer {
      buffer_ptr: ptr,
      size: size,
      position: AtomicUsize::new(0),
      layout: layout,
    })
  }
  
  pub fn slide_position(&self, delta: usize) {
    self.position.fetch_add(delta, Ordering::SeqCst);
  } 

  pub fn get_address(&self) -> *mut u8 {
    self.buffer_ptr
  }

  pub fn get_size(&self) -> usize {
    self.size
  }
  
  pub fn slice(&self) -> &[u8] {
    let pos = self.position.load(Ordering::SeqCst);
    unsafe {
      from_raw_parts((self.buffer_ptr as usize + pos) as *const u8, self.layout.size() - pos)
    }
  }
  
  pub fn slice_mut(&self) -> &mut [u8] {
    let pos = self.position.load(Ordering::SeqCst);
    unsafe {
      from_raw_parts_mut((self.buffer_ptr as usize + pos) as *mut u8, self.layout.size() - pos)
    }
  }
}
  
impl Drop for Buffer {
  fn drop(&mut self) {
    unsafe {
      dealloc(self.buffer_ptr, self.layout);
    }
  }
}
