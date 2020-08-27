
// ref: https://github.com/torvalds/linux/blob/master/Documentation/x86/boot.rst

use core::ffi::c_void;

#[repr(C)]
#[repr(packed)]
pub struct E820Entry {
    addr: u64,
    size: u64,
    entry_type: u32,
}

impl E820Entry {
  pub fn get_addr(&self) -> u64 {
    self.addr
  }

  pub fn get_size(&self) -> u64 {
    self.size
  }

  pub fn get_entry_type(&self) -> u32 {
    self.entry_type
  }

  pub fn get_entry_type_str(&self) -> &str {
    match self.entry_type {
      1 => "USABLE",
      2 => "RESERVED",
      3 => "ACPI",
      4 => "NVS",
      5 => "UNUSABLE",
      128 => "RESERVER_KERN",
      _ => "unknown",
    }
  }
}

pub unsafe fn get_boot_flag(boot_params: *const c_void) -> u16 {
  let offset = 0x01FE;
  *((boot_params as *const u8).offset(offset) as *const u16)
}
  
pub unsafe fn get_header(boot_params: *const c_void) -> u32 {
  let offset = 0x0202;
  *((boot_params as *const u8).offset(offset) as *const u32)
}
   
pub unsafe fn get_version(boot_params: *const c_void) -> u16 {
  let offset = 0x0206;
  *((boot_params as *const u8).offset(offset) as *const u16)
}

pub unsafe fn get_cmdline<'a>(boot_params: *const c_void) -> Option<&'a str> {
  let offset_cmd_line_ptr = 0x0228;
  let offset_cmd_line_size = 0x0238;
  let cmd_line_ptr = *((boot_params as *const u8).offset(offset_cmd_line_ptr) as *const u32) as *const u8;
  let cmd_line_size = *((boot_params as *const u8).offset(offset_cmd_line_size) as *const u32) as isize;

  for i in 0..cmd_line_size {
    if *cmd_line_ptr.offset(i) == 0 { //looking for NUL char
      return Some(core::str::from_utf8_unchecked(core::slice::from_raw_parts(cmd_line_ptr, i as usize)));
    }
  }
  None
}

pub unsafe fn get_e820<'a>(boot_params: *const c_void) -> &'a [self::E820Entry] {
  let offset_e820_entries = 0x01e8;
  let offset_e820_table = 0x02d0;
  let e820_entries = *((boot_params as *const u8).offset(offset_e820_entries) as *const u8) as usize;
  let ptr_e820_table = (boot_params as *const u8).offset(offset_e820_table) as *const E820Entry;
  
  &*(core::slice::from_raw_parts(ptr_e820_table, e820_entries))
}

