
use core::ptr;
use super::VirtioDevice;
use super::Virtqueue;

pub struct VirtioMMIO {
  addr: u64,
  size: usize,
  irq: u8,

  indirect_buf_cap: bool,
  event_idx_cap: bool,
}

impl VirtioMMIO {
  pub fn new(cmdline: &str, idx: usize) -> Result<VirtioMMIO, &str> {
    //assume that cmdline contains ".. virtio_mmio.device=4k@0xd0000000:5 "
    let keyphrase = "virtio_mmio.device=";

    let size_idx = {
      let mut partstr = &cmdline[0..];
      let mut keyidx = 0;
      for i in 0..(idx+1) {
        let key_idx = partstr.find(keyphrase);
        match key_idx {
          Some(j) => {
            keyidx += j+keyphrase.len();
            partstr = &cmdline[keyidx..]; //change range
          },
          None => {
            // keyphrase is not found anymore.
            return Err("virtio-mmio is not found");
          },
        }
        //println!("Virtio-MMIO: i={} idx={} partstr = \"{}\"", i, idx, partstr);
      }
      keyidx
    };

    //println!("Virtio-MMIO: partial string \"{}\"", &cmdline[size_idx..]);
    
    let addr_idx = match cmdline[size_idx..].find("@0x") {
      Some(idx) => size_idx + idx + 3,
      None => return Err("virtio-mmio is not found"),
    };

    let irq_idx = match cmdline[addr_idx..].find(':') {
      Some(idx) => addr_idx + idx + 1,
      None => return Err("virtio-mmio is not found"),
    };

    let tail_idx = match cmdline[irq_idx..].find(' ') {
      Some(idx) => irq_idx + idx,
      None => cmdline.len(),
    };
    
    let mut addr_val: u64 = 0;
    let mut size_val: usize = 0;
    let mut irq_val: u8 = 0;

    for c in cmdline[size_idx..addr_idx-3].chars() {
      size_val = match c {
        '0' => size_val*10 + 0,
        '1' => size_val*10 + 1,
        '2' => size_val*10 + 2,
        '3' => size_val*10 + 3,
        '4' => size_val*10 + 4,
        '5' => size_val*10 + 5,
        '6' => size_val*10 + 6,
        '7' => size_val*10 + 7,
        '8' => size_val*10 + 8,
        '9' => size_val*10 + 9,
        'k' | 'K' => size_val * 1024,
        'm' | 'M' => size_val * 1024 * 1024,
        _ => size_val,
      };
    }

    for c in cmdline[addr_idx..irq_idx-1].chars() {
      addr_val = match c {
        '0' => addr_val*16 + 0,
        '1' => addr_val*16 + 1,
        '2' => addr_val*16 + 2,
        '3' => addr_val*16 + 3,
        '4' => addr_val*16 + 4,
        '5' => addr_val*16 + 5,
        '6' => addr_val*16 + 6,
        '7' => addr_val*16 + 7,
        '8' => addr_val*16 + 8,
        '9' => addr_val*16 + 9,
        'a' | 'A' => addr_val*16 + 10,
        'b' | 'B' => addr_val*16 + 11,
        'c' | 'C' => addr_val*16 + 12,
        'd' | 'D' => addr_val*16 + 13,
        'e' | 'E' => addr_val*16 + 14,
        'f' | 'F' => addr_val*16 + 15,
        _ => addr_val,
      }
    }

    for c in cmdline[irq_idx..tail_idx].chars() {
      irq_val = match c {
        '0' => irq_val*10 + 0,
        '1' => irq_val*10 + 1,
        '2' => irq_val*10 + 2,
        '3' => irq_val*10 + 3,
        '4' => irq_val*10 + 4,
        '5' => irq_val*10 + 5,
        '6' => irq_val*10 + 6,
        '7' => irq_val*10 + 7,
        '8' => irq_val*10 + 8,
        '9' => irq_val*10 + 9,
        _ => irq_val,
      }
    }
    
    if unsafe { ptr::read_volatile(addr_val as *const u32) } != 0x74726976 {
      return Err("Invalid virtio-mmio device")
    }

    if unsafe { ptr::read_volatile((addr_val + 0x004) as *const u32) } != 0x2 {
      return Err("Invalid virtio-mmio device")
    }

    Ok(VirtioMMIO {addr: addr_val, size: size_val, irq: irq_val, event_idx_cap: false, indirect_buf_cap: false})
  }

  pub fn get_addr(&self) -> u64 {
    self.addr
  }

  pub fn get_size(&self) -> usize {
    self.size
  }

  pub fn get_irq(&self) -> u8 {
    self.irq
  }
}

impl VirtioDevice for VirtioMMIO {
  fn get_type(&self) -> u32 {
    unsafe { ptr::read_volatile((self.get_addr() + 0x008) as *const u32) } 
  }

  fn get_irq(&self) -> u8 {
    self.irq
  }

  fn read_and_ack_isr(&mut self) -> u8 {
    let intr_status = unsafe { ptr::read_volatile((self.get_addr() + 0x060) as *const u32) };
    unsafe {
      ptr::write_volatile((self.get_addr() + 0x064) as *mut u32, intr_status);
    }

    (intr_status & 0x01) as u8
  }

  fn select_queue(&mut self, queue: u32) {
    unsafe {
      ptr::write_volatile((self.get_addr() + 0x030) as *mut u32, queue);
      assert_eq!(ptr::read_volatile((self.get_addr() + 0x044) as *const u32), 0);
    }
  }

  fn get_queue_size(&self) -> u16 {
    unsafe { ptr::read_volatile((self.get_addr() + 0x034) as *const u32) as u16 }
  }

  fn setup_queue(&mut self, queue: &Virtqueue) {
    unsafe {
      ptr::write_volatile((self.get_addr() + 0x038) as *mut u32, queue.get_size() as u32);

      ptr::write_volatile((self.get_addr() + 0x080) as *mut u32, (queue.get_descriptor_addr() & 0xffffffff) as u32 );
      ptr::write_volatile((self.get_addr() + 0x084) as *mut u32, (queue.get_descriptor_addr() >> 32) as u32 );

      ptr::write_volatile((self.get_addr() + 0x090) as *mut u32, (queue.get_avail_addr() & 0xffffffff) as u32 );
      ptr::write_volatile((self.get_addr() + 0x094) as *mut u32, (queue.get_avail_addr() >> 32) as u32);

      ptr::write_volatile((self.get_addr() + 0x0a0) as *mut u32, (queue.get_used_addr() & 0xffffffff) as u32 );
      ptr::write_volatile((self.get_addr() + 0x0a4) as *mut u32, (queue.get_used_addr() >> 32) as u32 );
    }
  }

  fn activate_queue(&mut self, _queue: u32) {
    // assume that the queue provided has been selected.
    unsafe {
      ptr::write_volatile((self.get_addr() + 0x044) as *mut u32, 1);
    }
  }

  fn kick_queue(&mut self, queue: u32) {
    unsafe {
      ptr::write_volatile((self.get_addr() + 0x050) as *mut u32, queue);
    }
  }

  fn get_available_features(&mut self) -> u64 {
    // get higher part of features
    unsafe {
      ptr::write_volatile((self.get_addr() + 0x014) as *mut u32, 1);      
      let mut features = (ptr::read_volatile((self.get_addr() + 0x010) as *const u32) as u64) << 32;
      ptr::write_volatile((self.get_addr() + 0x014) as *mut u32, 0);
      features |= ptr::read_volatile((self.get_addr() + 0x010) as *const u32) as u64;

      features
    }
  }

  fn set_enabled_features(&mut self, features: u64) {
    unsafe { 
      ptr::write_volatile((self.get_addr() + 0x024) as *mut u32, 1);
      ptr::write_volatile((self.get_addr() + 0x020) as *mut u32, (features >> 32) as u32);
      ptr::write_volatile((self.get_addr() + 0x024) as *mut u32, 0);
      ptr::write_volatile((self.get_addr() + 0x020) as *mut u32, (features & 0xffffffff) as u32);
    }
  }

  fn get_status(&self) -> u8 {
    unsafe { ptr::read_volatile((self.get_addr() + 0x070) as *const u32) as u8 }
  }

  fn set_status(&mut self, status: u8) {
    unsafe {
      ptr::write_volatile((self.get_addr() + 0x070) as *mut u32, status as u32);
    }
  }

  fn read_config(&self, offset: usize) -> u8 {
    unsafe { ptr::read_volatile((self.get_addr() + 0x100 + offset as u64) as *const u8) }
  }

  fn get_shm(&self, _id: u8, _addr: u64, _length: usize) -> bool {
    false
  }

  fn is_legacy(&self) -> bool {
    false
  }

  fn set_indirect_buf_cap(&mut self, flag: bool) {
    self.indirect_buf_cap = flag;
  }
  fn get_indirect_buf_cap(&self) -> bool {
    self.indirect_buf_cap
  }
  fn set_event_idx_cap(&mut self, flag: bool) {
    self.event_idx_cap = flag;
  }
  fn get_event_idx_cap(&self) -> bool {
    self.event_idx_cap
  }
}

#[cfg(test)]
mod tests {
  /*
    #[test]
    fn exploration() {
        assert_eq!(2 + 2, 4);
    }
  */
}