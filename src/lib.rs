#![feature(asm)]
#![feature(llvm_asm)]
#![feature(const_fn)]
#![feature(const_btree_new)]
//#![feature(compiler_builtins)]
#![feature(alloc_error_handler)]
#![no_std]
//#![compiler_builtins]

#[macro_use]
mod console;
mod arch;
mod bootparams;
mod devices;
mod net;
mod interrupt;
mod spinlock;
mod asynchronous;
mod ringbuf;
mod task;

use core::panic::PanicInfo;
use core::ffi::c_void;
use core::alloc::Layout;
use core::mem::MaybeUninit;

use crate::console::*;
use crate::devices::serial;
use crate::devices::virtio;
use crate::devices::netif::Netif;
use crate::net::ProcessingNode;
use crate::net::ethernet::MacAddress;
use crate::net::ipv4::Ipv4Address;
use crate::net::ipv6::Ipv6Address;
use crate::interrupt::Interruptable;
use crate::spinlock::Spinlock;
use crate::asynchronous::executor::Executor;
use crate::asynchronous::timer::TimerFuture;
use crate::net::fib::{FIBType, MAC_ADDR_TABLE, IPV4_ADJACENT, AdjacentInformation, register_macaddress, register_ipv4_adjacent, register_ipv4_fib, register_ipv6_adjacent, register_ipv6_fib};

use crate::arch::x86_64::io;
use crate::arch::x86_64::apic;
use crate::arch::x86_64::kvmclock;
use crate::arch::x86_64::mptable;

use alloc::vec::Vec;
use alloc::sync::Arc;
use alloc::collections::BTreeMap;

extern crate crc;
extern crate alloc;
extern crate lock_api;
extern crate crossbeam_queue;
extern crate futures;
#[macro_use]
extern crate lazy_static;

pub static mut PROC_NODES: BTreeMap<&str, Arc<dyn ProcessingNode>> = BTreeMap::new();
pub static mut NET_IFACES: Vec<Arc<dyn Netif>> = Vec::new(); 
pub static mut EXECUTOR: Option<Executor> = None;

use linked_list_allocator::LockedHeap;

#[global_allocator]
pub static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[link(name = "kernel", kind = "static")]
#[no_mangle]
pub extern "C" fn start_rust(boot_params: *const c_void) {
  //assertion
  assert_eq!(unsafe { bootparams::get_boot_flag(boot_params) }, 0xaa55);
  assert_eq!(unsafe { bootparams::get_header(boot_params) }, 0x53726448);
  
  let cmd = match unsafe { bootparams::get_cmdline(boot_params) } {
    Some(cmdstr) => cmdstr,
    None => panic!("Couldn't get cmdline. something wrong."),
  };

  let e820 = unsafe { bootparams::get_e820(boot_params) };
  {
    //todo: get actual range
    let heap_start = 0x0000000000500000;
    let heap_end   = 0x000000003ff00000;
    let heap_size = heap_end - heap_start;
    unsafe {
      ALLOCATOR.lock().init(heap_start, heap_size);
    }
  }

  crate::console::init();

  println!("Booting Urchin ...");
  println!("Kernel cmdline: {}", cmd);
  println!("E820:");
  for ent in e820 {
    println!("  ADDR: {:016x}, SIZE: {:016x}, TYPE: {}", ent.get_addr(), ent.get_size(), ent.get_entry_type_str());
  }

  mptable::get_mp_table();

  {
    match kvmclock::init_kvmclock() {
      Ok(_) => println!("KVM Clock has been initialized."),
      Err(_) => println!("Failed to init KVM Clock."),
    }
    let calendar = kvmclock::get_calendar();
    println!(
      "Now, it's {:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", 
      calendar.year, calendar.month, calendar.day, calendar.hour, calendar.min, calendar.sec,
    );
  }

  //should i use APIC even if cmdline specify noapic?
  println!("Initialize APIC.");
  apic::init();

  println!("Initialize Asynchronous Executor.");
  unsafe {
    EXECUTOR = Some(Executor::new());
  }

  //set timer interrupt
  unsafe { interrupt::IRQ_HANDLERS.set_handler(Arc::new(interrupt::Timer::new())); }

  let setup_virtio_net = |index, ipv4addr, prefix| {
    let virtio_mmio = match virtio::mmio::VirtioMMIO::new(cmd, index) {
      Ok(inst) => inst,
      Err(msg) => {
        println!("Error: {}", msg);
        panic!(msg);
      }    
    };
    println!("Virtio-MMIO: addr:{:016x} size:{:016x} irq:{}", virtio_mmio.get_addr(), virtio_mmio.get_size(), virtio_mmio.get_irq());

    match virtio::net::VirtioNet::new(index, virtio_mmio) {
      Some(inst) => {
        let nif_arc = Arc::new(inst);
        unsafe {
          interrupt::IRQ_HANDLERS.set_handler(Arc::clone(&nif_arc) as Arc<dyn Interruptable>);
          NET_IFACES.push(Arc::clone(&nif_arc) as Arc<dyn Netif>);
        }
        {
          //register self mac address
          let macaddr = nif_arc.get_macaddress();
          register_macaddress(*macaddr, Arc::clone(&nif_arc) as Arc<dyn Netif + Send + Sync>, true, None);

          //
          register_ipv4_adjacent(ipv4addr, *macaddr, Arc::clone(&nif_arc) as Arc<dyn Netif + Send + Sync>, true, None);
          register_ipv4_fib(
            ipv4addr, 0xffffffff, *macaddr, ipv4addr, 
            Arc::clone(&nif_arc) as Arc<dyn Netif + Send + Sync>, FIBType::Local
          );
          register_ipv4_fib(
            ipv4addr.masked(24), 0xffffffffu32 << (32u32 - prefix), *macaddr, ipv4addr, 
            Arc::clone(&nif_arc) as Arc<dyn Netif + Send + Sync>, FIBType::Adjacent
          );

          // generate ipv6 ll addr
          let macaddr_array = macaddr.get_array();
          let lla_eui64 = Ipv6Address::from_array([
            0xfe, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            macaddr_array[0], macaddr_array[1], macaddr_array[2], 0xff, 
            0xfe, macaddr_array[3], macaddr_array[4], macaddr_array[5],
          ]);
          register_ipv6_adjacent(lla_eui64, *macaddr, Arc::clone(&nif_arc) as Arc<dyn Netif + Send + Sync>, true, None);
          register_ipv6_fib(
            lla_eui64, 128, *macaddr, lla_eui64, 
            Arc::clone(&nif_arc) as Arc<dyn Netif + Send + Sync>, FIBType::Local
          );

          // register ipv6 solicited node multicast
          let snmcast = Ipv6Address::from_array([
            0xff, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x01, 0xff, macaddr_array[3], macaddr_array[4], macaddr_array[5],
          ]);
          let snmcast_macaddr = MacAddress::new([0x33, 0x33, 0xff, macaddr_array[3], macaddr_array[4], macaddr_array[5],]);
          register_macaddress(snmcast_macaddr, Arc::clone(&nif_arc) as Arc<dyn Netif + Send + Sync>, true, None);
          register_ipv6_adjacent(snmcast, snmcast_macaddr, Arc::clone(&nif_arc) as Arc<dyn Netif + Send + Sync>, true, None);
          register_ipv6_fib(
            snmcast, 128, snmcast_macaddr, snmcast, 
            Arc::clone(&nif_arc) as Arc<dyn Netif + Send + Sync>, FIBType::Local
          );

          // register ipv6 all node multicast
          let allnodemcast = Ipv6Address::from_array([
            0xff, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
          ]);
          let allnodemcast_macaddr = MacAddress::new([0x33, 0x33, 0x00, 0x00, 0x00, 0x01,]);
          register_macaddress(allnodemcast_macaddr, Arc::clone(&nif_arc) as Arc<dyn Netif + Send + Sync>, true, None);
          register_ipv6_adjacent(allnodemcast, allnodemcast_macaddr, Arc::clone(&nif_arc) as Arc<dyn Netif + Send + Sync>, true, None);
          register_ipv6_fib(
            allnodemcast, 128, allnodemcast_macaddr, allnodemcast, 
            Arc::clone(&nif_arc) as Arc<dyn Netif + Send + Sync>, FIBType::Local
          );
        }
      },
      None => (),
    };
  };

  setup_virtio_net(0, Ipv4Address::from_array([192, 168, 0, 10]), 24);
  setup_virtio_net(1, Ipv4Address::from_array([192, 168, 10, 10]), 24);

  //todo: choose nodes as user feels like.
  {
    let ether_in = Arc::new(net::ethernet::EthernetIn::new());
    unsafe {
      PROC_NODES.insert("ethernet-in", ether_in as Arc<dyn ProcessingNode>);
    }
    let arp_in = Arc::new(net::arp::ArpIn::new());
    unsafe {
      PROC_NODES.insert("arp-in", arp_in as Arc<dyn ProcessingNode>);
    }
    //ipv4
    let ipv4_in = Arc::new(net::ipv4::Ipv4In::new());
    unsafe {
      PROC_NODES.insert("ipv4-in", ipv4_in as Arc<dyn ProcessingNode>);
    }
    let icmpv4_in = Arc::new(net::ipv4::Icmpv4InLocal::new());
    unsafe {
      PROC_NODES.insert("icmpv4-in-local", icmpv4_in as Arc<dyn ProcessingNode>);
    }
    //ipv6
    let ipv6_in = Arc::new(net::ipv6::Ipv6In::new());
    unsafe {
      PROC_NODES.insert("ipv6-in", ipv6_in as Arc<dyn ProcessingNode>);
    }
    let icmpv6_in = Arc::new(net::ipv6::Icmpv6InLocal::new());
    unsafe {
      PROC_NODES.insert("icmpv6-in-local", icmpv6_in as Arc<dyn ProcessingNode>);
    }

  }

  ////// codes below here are dummy

  /*
  let nif_opt = unsafe { netifs.pop() };
  if let Some(nif) = nif_opt {
    let macaddr = nif.get_macaddress();
    {
      let macaddr_array = macaddr.get_array();
      println!(
        "MAC Address = {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}", 
        macaddr_array[0], macaddr_array[1], macaddr_array[2], macaddr_array[3], macaddr_array[4], macaddr_array[5]
      );
    }
    let mut debug_buf = [0u8; 64];
    let myip = crate::net::ipv4::Ipv4Address::from_array([192, 168, 0, 10]);
    let destip = crate::net::ipv4::Ipv4Address::from_array([192, 168, 0, 15]);
    crate::net::generate_arp_req(&mut debug_buf, &macaddr, &myip, &destip);
    loop {
      nif.xmit(&debug_buf);

      println!("Emit ARP");
  
      for _i in 0..10_000_000_000u64 {
        unsafe {
          io::nop();
        }
      }
    }  
  }
  */
  println!("/*-- Summary of Configuration ----");
  println!("Interfaces:");
  for (i, netif) in unsafe { NET_IFACES.iter().enumerate() } {
    let macaddr_array = netif.get_macaddress().get_array();
    println!(
      "  {:4}  driver={} MACAddress={:02x}-{:02x}-{:02x}-{:02x}-{:02x}-{:02x}", 
      i, netif.get_drivername(), 
      macaddr_array[0], macaddr_array[1], macaddr_array[2], macaddr_array[3], macaddr_array[4], macaddr_array[5],
    );
  }
  println!("---- Summary of Configuration --*/");

  //add test task
  if let Some(exec) = unsafe { EXECUTOR.as_ref() } {
    exec.spawn(async {
      use core::time::Duration;
      loop {
        let calendar = kvmclock::get_calendar();
        println!(
          "TestTask1: Now, it's {:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", 
          calendar.year, calendar.month, calendar.day, calendar.hour, calendar.min, calendar.sec,
        );
    
        TimerFuture::new(Duration::new(5, 0)).await
      }
    });
    exec.spawn(async {
      use core::time::Duration;
      loop {
        let calendar = kvmclock::get_calendar();
        println!(
          "TestTask2: Now, it's {:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", 
          calendar.year, calendar.month, calendar.day, calendar.hour, calendar.min, calendar.sec,
        );
    
        TimerFuture::new(Duration::new(8, 0)).await
      }
    });
  }


  println!("Launched");

  loop {
    // concentrate to finish tasks
    unsafe { interrupt::disable_interrupt(); }
    if let Some(exec) = unsafe { EXECUTOR.as_ref() } {
      exec.run();
    }
    unsafe { interrupt::enable_interrupt(); }

    // go to halt after all tasks for now has been done.
    unsafe {
      io::hlt();
    }
  }
}

//should be refactored
#[repr(C, packed)]
pub struct InterruptFrame {
  rax: u64,
  rbx: u64,
  rcx: u64,
  rdx: u64,
  rsi: u64,
  rbp: u64,
  r8: u64,
  r9: u64,
  r10: u64,
  r11: u64,
  r12: u64,
  r13: u64,
  r14: u64,
  r15: u64,
  xmm0: u128,
  xmm1: u128,
  xmm2: u128,
  xmm3: u128,
  xmm4: u128,
  xmm5: u128,
  xmm6: u128,
  xmm7: u128,
  xmm8: u128,
  xmm9: u128,
  xmm10: u128,
  xmm11: u128,
  xmm12: u128,
  xmm13: u128,
  xmm14: u128,
  xmm15: u128,
  rdi: u64,
  error: u64,
  rip: u64,
  cs: u64,
  rflags: u64,
  rsp: u64,
  ss: u64,
}

#[link(name = "kernel", kind = "static")]
#[no_mangle]
pub extern "C" fn interrupt_handler(intvec: u8, frame: *const InterruptFrame) {
  let frm = unsafe { &*frame };
  let rip = unsafe { frm.rip };
  let rsp = unsafe { frm.rsp };
  
  if intvec <= 20 {
    println!("Interrupted. vector = {}, FRAME = {:016x}, RIP = {:016x} RSP = {:016x}", intvec, frame as usize, rip, rsp);
    println!("Couldn't rescue. panic.");
    panic!();
  } else if intvec >= 48 {
    interrupt::irq_handler(intvec - 48);
  }
}

#[cfg(not(test))]
#[no_mangle]
#[panic_handler]
fn panic(panic_info: &PanicInfo) -> ! {
  //TODO: add output panic information
  if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
    println!("Panic! : {:?}", s);
  } else {
    println!("Panic!");
  }

  loop {
    unsafe {
      io::hlt();
    }
  }
}

#[cfg(not(test))]
#[alloc_error_handler]
fn on_oom(_layout: Layout) -> ! {
  panic!("OOM.");
}

//////////////////
// thanks to https://github.com/rust-lang/compiler-builtins/blob/master/src/mem.rs

#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    let mut i = 0;
    while i < n {
        let a = *s1.offset(i as isize);
        let b = *s2.offset(i as isize);
        if a != b {
            return a as i32 - b as i32;
        }
        i += 1;
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn bcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    memcmp(s1, s2, n)
}

type CInt = i32;

#[no_mangle]
pub unsafe extern "C" fn memset(s: *mut u8, c: CInt, n: usize) -> *mut u8 {
  let mut i = 0;
  while i < n {
      *s.offset(i as isize) = c as u8;
      i += 1;
  }
  s
}

#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut i = 0;
    while i < n {
        *dest.offset(i as isize) = *src.offset(i as isize);
        i += 1;
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if src < dest as *const u8 {
        // copy from end
        let mut i = n;
        while i != 0 {
            i -= 1;
            *dest.offset(i as isize) = *src.offset(i as isize);
        }
    } else {
        // copy from beginning
        let mut i = 0;
        while i < n {
            *dest.offset(i as isize) = *src.offset(i as isize);
            i += 1;
        }
    }
    dest
}