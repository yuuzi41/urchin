use core::ptr;
use super::io;

const APIC_REG_APICID: u64 = 0xfee00020u64;
const APIC_REG_TPR: u64 = 0xfee00080u64;
const APIC_REG_EOI: u64 = 0xfee000b0u64;
const APIC_REG_LOGICAL_DEST: u64 = 0xfee000d0u64;
const APIC_REG_SPURIOUS_INT: u64 = 0xfee000f0u64;
const APIC_REG_DEST_FORMAT: u64 = 0xfee000e0u64;
const APIC_REG_LVT_TIMER: u64 = 0xfee00320u64;
const APIC_REG_LVT_ERROR: u64 = 0xfee00370u64;
const APIC_REG_TIMER_INITCNT: u64 = 0xfee00380u64;
const APIC_REG_TIMER_CURCNT: u64 = 0xFEE00390u64;
const APIC_REG_TIMER_DIV: u64 = 0xfee003e0u64;

const VECTOR_IPI_RESCHEDULE: u32 = 32;
const VECTOR_IPI_HALT: u32 = 33;
const VECTOR_IRQ_BASE: u32 = 48;
const TIMER_IRQ: u32 = 0;
const TIMER_PERIODIC: u32 = 0x20000;

const IOAPIC_ADDR: u64 = 0xfec00000u64;
const IOAPIC_REG_IOAPICVER: u8 = 0x01u8;

//
//  APIC Timer.
//
const APIC_TIMER_DIV: u32 = 0x03;

#[repr(C, packed)]
struct IDTRegister {
  limit: u16,
  base: u64,
}

impl IDTRegister {
  const fn new() -> IDTRegister {
    IDTRegister {limit: 0, base: 0}
  }

  fn set_limit(&mut self, limit: u16) {
    self.limit = limit;
  }

  fn set_base(&mut self, base: u64) {
    self.base = base;
  }
}

#[derive(Copy, Clone, Debug)]
#[repr(packed)]
pub struct IDTEntry {
  offsetl: u16,
  selector: u16,
  zero: u8,
  attribute: u8,
  offsetm: u16,
  offseth: u32,
  zero2: u32
}

impl IDTEntry {
  const fn new() -> IDTEntry {
    IDTEntry {
      offsetl: 0,
      selector: 0,
      zero: 0,
      attribute: 0,
      offsetm: 0,
      offseth: 0,
      zero2: 0,
    }
  }
  fn set_offset(&mut self, offset: u64) {
    self.offsetl = ((offset & 0x000000000000ffff) >> 0) as u16;
    self.offsetm = ((offset & 0x00000000ffff0000) >> 16) as u16;
    self.offseth = ((offset & 0xffffffff00000000) >> 32) as u32;

    self.attribute = 0x8e; //PRESENT, RING0, INTERRUPT
    self.selector = 8; // KERNEL_CS
  }
}

extern {
  fn interrupt_handlers();
}

static mut IDTR: IDTRegister = IDTRegister::new();
static mut IDT: [IDTEntry; 256] = [IDTEntry::new(); 256];


//KVM paravirtualized EOI
const KVM_PV_EOI_ENABLED: u64 = 1;
const KVM_PV_EOI_DISABLED: u64 = 0;
static mut KVM_PV_EOI: [u32; 256] = [0; 256];
const MSR_KVM_PV_EOI_EN: u32 = 0x4b564d04;

pub fn init() {
  unsafe {
    // disable 8259 PIC
    io::outb(0x00a1, 0xff);
    io::outb(0x0021, 0xff);
  }
  unsafe {
    // enable Local APIC
    const MSR_APIC_BASE: u32 = 0x0000001bu32;
    io::wrmsr(MSR_APIC_BASE, (io::rdmsr(MSR_APIC_BASE) & 0xfffff100) | 0x0800);
  }

  write(APIC_REG_SPURIOUS_INT, 1 << 8);
  write(APIC_REG_TPR, 0);
  write(APIC_REG_LOGICAL_DEST, 0x01000000u32);
  write(APIC_REG_DEST_FORMAT, 0xffffffffu32);
  write(APIC_REG_LVT_TIMER, 1 << 16 /* masked */);
  write(APIC_REG_LVT_ERROR, 1 << 16 /* masked */);

  {
    // set up timer
    //todo: use other clock
    println!("APIC: Set LAPIC Timer");

    use crate::arch::x86_64::kvmclock::get_monotonic_time;

    write(APIC_REG_TIMER_INITCNT, 0xffffffff);
    write(APIC_REG_TIMER_DIV, APIC_TIMER_DIV);

    let before = get_monotonic_time();
    loop {
      if read(APIC_REG_TIMER_CURCNT) < 0xfff00000 {
        break;
      }
    }
    let after = get_monotonic_time();
    assert!(after > before);

    let base_cnt = (2 * 1_000_000_000 / 1_000) * 0xfffff / (after - before); // 2ms

    //println!("APIC: DEBUG before={} after={} diff={} CNT={}", before, after, after - before, base_cnt);

    write(APIC_REG_TIMER_INITCNT, base_cnt as u32);
    write(APIC_REG_LVT_TIMER, (VECTOR_IRQ_BASE + TIMER_IRQ) | TIMER_PERIODIC);
  }

  unsafe {
    //set idt
    for i in 0..256 {
      IDT[i].set_offset((interrupt_handlers as u64) + (0x10u64 * i as u64));
    }
    {
      let idt_reg = &mut IDTR;
      idt_reg.set_limit(16 * 256 - 1);
      idt_reg.set_base(IDT.as_ptr() as u64);
    }
    llvm_asm!("lidt ($0)" :: "r" (&IDTR) : "memory");
  }

  const KVM_FEATURE_PV_EOI: u32 = 6;
  if (io::get_kvm_features() & (1 << KVM_FEATURE_PV_EOI)) != 0 {
    let addr = unsafe { (&KVM_PV_EOI[get_lapic_id() as usize] as *const u32) as u64 };
    unsafe {
      io::wrmsr(MSR_KVM_PV_EOI_EN, addr | KVM_PV_EOI_ENABLED);
    }
  }

  ioapic_init();
}

pub fn read(addr: u64) -> u32 {
  unsafe {
    let pointer = addr as *const u32;
    ptr::read_volatile(pointer)
  }
}

pub fn write(addr: u64, val: u32) {
  unsafe {
    let pointer = addr as *mut u32;
    ptr::write_volatile(pointer, val);
  }
}

pub fn get_lapic_id() -> u8 {
  (read(APIC_REG_APICID) >> 24) as u8
}

//////////

fn ioapic_reg_low(n: u8) -> u8 {
  (0x10 + ((n) * 2)) as u8
}
fn ioapic_reg_high(n: u8) -> u8 {
  (0x10 + ((n) * 2) + 1) as u8
}

fn ioapic_read(reg: u8) -> u32 {
  let reg_pointer = (IOAPIC_ADDR + 0x00) as *mut u32;
  let dat_pointer = (IOAPIC_ADDR + 0x10) as *const u32;

  unsafe {
    ptr::write_volatile(reg_pointer, reg as u32);
    ptr::read_volatile(dat_pointer)
  }
}

fn ioapic_write(reg: u8, data: u32) {
  let reg_pointer = (IOAPIC_ADDR + 0x00) as *mut u32;
  let dat_pointer = (IOAPIC_ADDR + 0x10) as *mut u32;
  
  unsafe {
    ptr::write_volatile(reg_pointer, reg as u32);
    ptr::write_volatile(dat_pointer, data);
  }
}

pub fn ioapic_init() {
  unsafe {
    io::outb(0x0022, 0x70);
    io::outb(0x0023, 0x01);
  }

  // disable all interrupts
  let num = (ioapic_read(IOAPIC_REG_IOAPICVER) >> 16) + 1;
  for i in 0..num {
    ioapic_write(ioapic_reg_high(i as u8), 0);
    ioapic_write(ioapic_reg_low(i as u8), 1 << 16);
  }

  ack_irq();
}

pub fn enable_irq(irq: u8) {
  ioapic_write(ioapic_reg_high(irq), 0);
  ioapic_write(ioapic_reg_low(irq), VECTOR_IRQ_BASE + irq as u32);
}

pub fn disable_irq(irq: u8) {
  ioapic_write(ioapic_reg_high(irq), 0);
  ioapic_write(ioapic_reg_low(irq), 1 << 16);
}

pub fn ack_irq() {
  let ret: u8;
  unsafe {
    asm!(
      "btr [{}], {}",
      "setc {}",
      in(reg) &KVM_PV_EOI[get_lapic_id() as usize],
      in(reg) 0u32,
      out(reg_byte) ret,
    );
  }
  if ret == 0 {
    write(APIC_REG_EOI, 0);
  }
}
