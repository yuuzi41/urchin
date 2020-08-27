pub unsafe fn outb(port: u16, val: u8) {
  asm!("out dx, al", in("al") val, in("dx") port);
}

pub unsafe fn outw(port: u16, val: u16) {
  asm!("out dx, ax", in("ax") val, in("dx") port);
}

pub unsafe fn outl(port: u16, val: u32) {
  asm!("out dx, eax", in("eax") val, in("dx") port);
}

pub unsafe fn inb(port: u16) -> u8 {
  let ret: u8;
  asm!("in al, dx", out("al") ret, in("dx") port);
  ret
}

pub unsafe fn inw(port: u16) -> u16 {
  let ret: u16;
  asm!("in ax, dx", out("ax") ret, in("dx") port);
  ret
}

pub unsafe fn inl(port: u16) -> u32 {
  let ret: u32;
  asm!("in eax, dx", out("eax") ret, in("dx") port);
  ret
}

pub unsafe fn hlt() {
  asm!("hlt");
}

pub unsafe fn nop() {
  asm!("nop");
}

pub fn rdtsc_with_cpuid() -> u64 {
  unsafe {
    let hi: u32;
    let lo: u32;
    asm!(
      "cpuid",
      "mfence",
      "rdtsc",
      out("eax") lo,
      out("edx") hi,
    );
    (hi as u64) << 32 | (lo as u64)
  }
}

pub fn get_kvm_features() -> u32 {
  unsafe {
    let ret: u32;
    asm!(
      "cpuid",
      inout("eax") 0x40000001 => ret,
    );
    ret
  }
}

pub fn cpuid(eax: u32) -> (u32, u32, u32, u32) {
  unsafe {
    let ret_a: u32;
    let ret_b: u32;
    let ret_c: u32;
    let ret_d: u32;
    asm!(
      "cpuid",
      inout("eax") eax => ret_a,
      out("ebx") ret_b,
      out("ecx") ret_c,
      out("edx") ret_d,
    );
    (ret_a, ret_b, ret_c, ret_d)
  }
}

pub unsafe fn enable_interrupt() {
  asm!("sti");
}

pub unsafe fn disable_interrupt() {
  asm!("cli");
}

pub unsafe fn wrmsr(reg: u32, val: u64) {
  let lo = (val & 0xffffffff) as u32;
  let hi = (val >> 32) as u32;
  asm!("wrmsr", in("ecx") reg, in("eax") lo, in("edx") hi);
}

pub unsafe fn rdmsr(reg: u32) -> u64 {
  let lo: u32;
  let hi: u32;
  asm!("rdmsr", in("ecx") reg, out("eax") lo, out("edx") hi);
  (hi as u64) << 32 | (lo as u64)
}
