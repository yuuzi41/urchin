pub mod mmio;
pub mod net;

use core::slice;
use alloc::vec;
use alloc::alloc::{alloc, dealloc, Layout};
use alloc::vec::Vec;
use alloc::sync::Arc;

use crate::devices::buffer::Buffer;

pub trait VirtioDevice {
  fn get_type(&self) -> u32;

  fn get_irq(&self) -> u8;
  fn read_and_ack_isr(&mut self) -> u8;

  fn select_queue(&mut self, queue: u32);
  fn get_queue_size(&self) -> u16;
  fn setup_queue(&mut self, queue: &Virtqueue);
  fn activate_queue(&mut self, queue: u32);
  fn kick_queue(&mut self, queue: u32);

  fn get_available_features(&mut self) -> u64;
  fn set_enabled_features(&mut self, features: u64);

  fn get_status(&self) -> u8;
  fn set_status(&mut self, status: u8);

  fn read_config(&self, offset: usize) -> u8;
  fn get_shm(&self, id: u8, addr: u64, length: usize) -> bool;

  fn is_legacy(&self) -> bool;

  fn set_indirect_buf_cap(&mut self, flag: bool);
  fn get_indirect_buf_cap(&self) -> bool;
  fn set_event_idx_cap(&mut self, flag: bool);
  fn get_event_idx_cap(&self) -> bool;
}

#[repr(C,packed)]
struct VirtqueueDescriptor { 
  addr: u64, 
  len: u32,
  flags: u16, 
  next: u16,
}

#[repr(C,packed)]
struct VirtqueueAvail { 
  flags: u16,
  idx: u16,
}

#[repr(C,packed)]
struct VirtqueueUsed { 
  flags: u16,
  idx: u16,
}

#[repr(C,packed)]
struct VirtqueueUsedElement { 
  id: u32,
  len: u32,
}

pub struct Virtqueue<'a> {
  base_addr: u64,
  layout: Layout,
  size: u16,

  descriptor: &'a mut [VirtqueueDescriptor],
  avail: &'a mut VirtqueueAvail,
  avail_ring: &'a mut [u16],
  used: &'a mut VirtqueueUsed,
  used_ring: &'a mut [VirtqueueUsedElement],

  buffers: Vec<Option<Arc<Buffer>>>,

  free_head: u16,
  last_used_idx: u16,

  is_indirect: bool, 
}

impl<'a> Drop for Virtqueue<'a> {
  fn drop(&mut self) {
    unsafe {
      dealloc(self.base_addr as *mut u8, self.layout);
    }
  }
}

impl<'a> Virtqueue<'a> {
  pub fn new(size: u16, is_indirect: bool, used_interrupt: bool) -> Virtqueue<'a> {
    unsafe {
      let align = 4096usize;
      let alloc_size = 
        ((16 * size as usize + 3 * 2 + 2 * size as usize - 1) / align + 1) * align + 
        ((3 * 2 + 8 * size as usize - 1) / align + 1) * align;

      let layout = match Layout::from_size_align(alloc_size, align) {
        Ok(l) => l,
        Err(_e) => panic!("Alignment failed.")
      };

      let base_addr = alloc(layout) as u64;

      let descriptor_addr = base_addr;
      let avail_addr = base_addr + 16 * size as u64;
      let used_addr = base_addr + (((16 * size as usize + 3 * 2 + 2 * size as usize - 1) / align + 1) * align) as u64;

      let descriptor = slice::from_raw_parts_mut(descriptor_addr as *mut VirtqueueDescriptor, size as usize);
      let avail = &mut *(avail_addr as *mut VirtqueueAvail);
      let avail_ring = slice::from_raw_parts_mut((avail_addr + 4) as *mut u16, size as usize);
      let used = &mut *(used_addr as *mut VirtqueueUsed);
      let used_ring = slice::from_raw_parts_mut((used_addr + 4) as *mut VirtqueueUsedElement, size as usize);

      avail.flags = match used_interrupt {
        true => 0, // INTERRUPT
        false => 1, // want device not to INTERRUPT
      }; 
      avail.idx = 0;
      used.flags = 0; // NOTIFY
      used.idx = 0;

      for (_i, desc) in descriptor.iter_mut().enumerate() {
        desc.addr = 0;
        desc.len = 0;
        desc.flags = 0;
        desc.next = 0;
      }

      Virtqueue {
        base_addr: base_addr,
        layout: layout,
        size: size,

        descriptor: descriptor,
        avail: avail,
        avail_ring: avail_ring,
        used: used,
        used_ring: used_ring,

        buffers: vec![None; size as usize],

        free_head: 0,
        last_used_idx: 0,
        is_indirect: is_indirect, 
      }
    }
  }

  fn get_descriptor_addr(&self) -> u64 {
    self.base_addr
  }

  fn get_avail_addr(&self) -> u64 {
    self.base_addr + 16 * self.get_size() as u64
  }

  fn get_used_addr(&self) -> u64 {
    let align = 4096usize;
    self.base_addr + (((16 * self.get_size() as usize + 3 * 2 + 2 * self.get_size() as usize - 1) / align + 1) * align) as u64
  }

  fn get_size(&self) -> u16 {
    self.size as u16
  }

  fn get_avail_free_size(&self) -> u16 {
    if self.avail.idx >= self.used.idx {
      (self.get_size() - (self.avail.idx - self.used.idx))
    } else {
      (self.used.idx - self.avail.idx)
    }
    //(self.get_size() - self.avail.idx as u32 + self.used.idx as u32 - self.last_used_idx)
  }

  fn push(&mut self, buffer: Arc<Buffer>, is_guest: bool) -> Result<(), &'static str> {
    //todo: lock
    let desc_idx = self.free_head as usize;

    //set avail ring
    self.avail_ring[(self.avail.idx & (self.get_size() - 1)) as usize] = desc_idx as u16;
    self.avail.idx = self.avail.idx + 1;

    //this is not so good due to efficiency.
    {
      let desc = &mut self.descriptor[desc_idx];
      desc.addr = buffer.get_address() as u64;
      desc.len = buffer.get_size() as u32;
      if is_guest {
        desc.flags = 0; //no next, device read only, no indirect
      } else {
        desc.flags = 2; //no next, device write only, no indirect
      }

      //to keep this buffer. at the same time old buffer may be dropped.
      self.buffers[desc_idx] = Some(Arc::clone(&buffer));

      desc.next = (desc_idx as u16 + 1) & (self.size - 1);
    }

    self.free_head = (desc_idx as u16 + 1) & (self.size - 1);

    //println!("PUSH avail.idx={} used.idx={} addr={:016x} size={} desc_idx={}", self.avail.idx, self.used.idx, buffer.get_address() as u64, buffer.get_size(), desc_idx);

    Ok(())
  }

  fn pop(&mut self) -> Vec<Arc<Buffer>> {
    let mut ret = Vec::new();
 
    while self.last_used_idx != self.used.idx {
      let desc_idx = self.used_ring[(self.last_used_idx & (self.get_size() - 1)) as usize].id as usize;
      {
        let desc = &self.descriptor[desc_idx];
        let addr = (desc.addr) as *const u8;
        let len = self.used_ring[(self.last_used_idx & (self.get_size() - 1)) as usize].len as usize;
        let buf = unsafe { slice::from_raw_parts(addr, len) };
      }
      if let Some(buff) = self.buffers[desc_idx].as_ref() {
        ret.push(Arc::clone(buff));
      }
      self.buffers[desc_idx] = None;
      self.last_used_idx = self.last_used_idx + 1;
    }
    //println!("POP avail.idx={} used.idx={}", self.avail.idx, self.used.idx);

    ret
  }
}
