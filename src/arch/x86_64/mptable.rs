
#[repr(C,packed)]
pub struct MpfStructure {
  signature: [u8; 4], // "_MP_"
  configuration_table: u32,
  length: u8,    // In 16 bytes (e.g. 1 = 16 bytes, 2 = 32 bytes)
  specification_revision: u8, 
  checksum: u8,  // This value should make all bytes in the table equal 0 when added together
  default_configuration: u8, // If this is not zero then configuration_table should be
                             // ignored and a default configuration should be loaded instead
  features: u32, // If bit 7 is then the IMCR is present and PIC mode is being used, otherwise
                 // virtual wire mode is; all other bits are reserved
}

#[repr(C,packed)]
pub struct MpTable {
  signature: [u8; 4], // "PCMP"
  length: u16,
  mp_specification_revision: u8,
  checksum: u8,  // Again, the byte should be all bytes in the table add up to 0
  oem_id: [u8; 8],
  product_id: [u8; 12],
  oem_table: u32,
  oem_table_size: u16,
  entry_count: u16,   // This value represents how many entries are following this table
  lapic_address: u32, // This is the memory mapped address of the local APICs
  extended_table_length: u16,
  extended_table_checksum: u8,
  reserved: u8,
}

#[repr(C,packed)]
pub struct MpProcessor {
  entry_type: u8,  // Always 0
  local_apic_id: u8,
  local_apic_version: u8,
  flags: u8, // If bit 0 is clear then the processor must be ignored
                 // If bit 1 is set then the processor is the bootstrap processor
  signature: u32,
  feature_flags: u32,
  reserved: u64,
}

pub fn find_mp_table(base_addr: u32, length: usize) -> Option<&'static MpTable> {
  for i in 0..(length / 16) {
    let mpf_candidate = unsafe { &*((base_addr + (i as u32) * 16u32) as *const MpfStructure) };
    if mpf_candidate.signature == [b'_', b'M', b'P', b'_'] {
      let mp_candidate = unsafe { &*(mpf_candidate.configuration_table as *const MpTable) };
      if mp_candidate.signature == [b'P', b'C', b'M', b'P'] {
        return Some(mp_candidate);
      }
    }
  }
  None
}

pub fn get_mp_table() {
  const LAST_KB_IN_BASE_MEMORY_ADDR: u32 = 639 * 0x400;
  const FIRST_KB_IN_BASE_MEMORY_ADDR: u32 = 0;
  const MP_FIND_LENGTH: usize = 0x400;

  const NON_PROCESSOR_ENTRY_SIZE: u64 = 8;
  const PROCESSOR_ENTRY_SIZE: u64 = 20;
  

  let mptable_opt = match find_mp_table(LAST_KB_IN_BASE_MEMORY_ADDR, MP_FIND_LENGTH) {
    Some(mptable_ref) => Some(mptable_ref),
    None => find_mp_table(FIRST_KB_IN_BASE_MEMORY_ADDR, MP_FIND_LENGTH),
  };

  let mut num_of_cpus = 0u32;
  if let Some(mptable) = mptable_opt {
    println!("MP Table is found. OEM_ID=\"{}\" PRODUCT_ID=\"{}\"", 
      unsafe { &*(&mptable.oem_id as *const [u8] as *const str) }, 
      unsafe { &*(&mptable.product_id as *const [u8] as *const str) }, 
    );

    let mut entries_length = (mptable.length - 44) as i64;
    let mut base_addr = ((mptable as *const MpTable) as u64) + 44;
    while entries_length > 0 {
      let mpproc = unsafe { &*(base_addr as *const MpProcessor) };
      match mpproc.entry_type {
        0 => {
          println!("CPU {}, APIC_ID={}", num_of_cpus, mpproc.local_apic_id);
          num_of_cpus = num_of_cpus + 1;
          entries_length = entries_length - PROCESSOR_ENTRY_SIZE as i64;
          base_addr = base_addr + PROCESSOR_ENTRY_SIZE;
        },
        _ => {
          entries_length = entries_length - NON_PROCESSOR_ENTRY_SIZE as i64;
          base_addr = base_addr + NON_PROCESSOR_ENTRY_SIZE;
        },
      }
    }
  }

  println!("{} CPUs detected.", num_of_cpus);
}