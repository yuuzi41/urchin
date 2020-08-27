use core::sync::atomic::{Ordering, fence};
use core::ptr::read_volatile;

use crate::arch::x86_64::io;

#[repr(C,packed)]
struct PVClockVcpuTimeInfo {
  version: u32,
  pad0: u32,
  tsc_timestamp: u64, 
  system_time: u64,
  tsc_to_system_mul: u32,
  tsc_shift: i8,
  flags: u8,
  pad1: u16,
}

impl PVClockVcpuTimeInfo {
  const fn new() -> PVClockVcpuTimeInfo {
    PVClockVcpuTimeInfo {
      version: 0,
      pad0: 0,
      tsc_timestamp: 0,
      system_time: 0,
      tsc_to_system_mul: 0,
      tsc_shift: 0,
      flags: 0,
      pad1: 0,
    }
  }
}

#[repr(C,packed)]
struct PVClockWallClock {
  version: u32,
  sec: u32,
  nsec: u32,
}

impl PVClockWallClock {
  const fn new() -> PVClockWallClock {
    PVClockWallClock {
      version: 0,
      sec: 0,
      nsec: 0,
    }
  }
}

static KVM_CLOCK_INFO_PER_VCPU: PVClockVcpuTimeInfo = PVClockVcpuTimeInfo::new();
static KVM_CLOCK_WALL_CLOCK: PVClockWallClock = PVClockWallClock::new();

const MSR_KVM_WALL_CLOCK: u32 = 0x11;
const MSR_KVM_SYSTEM_TIME: u32 = 0x12;

const MSR_KVM_WALL_CLOCK_NEW: u32 = 0x4b564d00;
const MSR_KVM_SYSTEM_TIME_NEW: u32 = 0x4b564d01;


// todo: supporting multicore
pub fn init_kvmclock() -> Result<(),()> {
  const KVM_FEATURE_CLOCKSOURCE: u32 = 0;
  const KVM_FEATURE_CLOCKSOURCE2: u32 = 3;
  let features = io::get_kvm_features();

  let wallclock_address = unsafe { ((&KVM_CLOCK_WALL_CLOCK) as *const PVClockWallClock) as u64 };
  let clockinfo_address = unsafe { ((&KVM_CLOCK_INFO_PER_VCPU) as *const PVClockVcpuTimeInfo) as u64 } | 1u64; // setting the first bit is necessery

  if (features & (1 << KVM_FEATURE_CLOCKSOURCE2)) != 0 {
    unsafe {
      io::wrmsr(MSR_KVM_WALL_CLOCK_NEW, wallclock_address);
      io::wrmsr(MSR_KVM_SYSTEM_TIME_NEW, clockinfo_address);
    }
  } else if (features & (1 << KVM_FEATURE_CLOCKSOURCE)) != 0 {
    unsafe {
      io::wrmsr(MSR_KVM_WALL_CLOCK, wallclock_address);
      io::wrmsr(MSR_KVM_SYSTEM_TIME, clockinfo_address);
    }
  } else {
    return Err(());
  }
  Ok(())
}

// nanosec
pub fn get_monotonic_time() -> u64 {
  let nowtsc = io::rdtsc_with_cpuid();

  let mut ret;
  loop {
    let old_val = unsafe { read_volatile(&KVM_CLOCK_INFO_PER_VCPU as *const PVClockVcpuTimeInfo) };
    fence(Ordering::SeqCst);
    let tsc_delta = (nowtsc - old_val.tsc_timestamp) as u128;
    let shifted_delta = if old_val.tsc_shift >= 0 {
      tsc_delta << old_val.tsc_shift
    } else {
      tsc_delta >> -old_val.tsc_shift
    };
    ret = ((shifted_delta * old_val.tsc_to_system_mul as u128) >> 32) as u64;
    fence(Ordering::SeqCst);
    let new_val = unsafe { read_volatile(&KVM_CLOCK_INFO_PER_VCPU as *const PVClockVcpuTimeInfo) };

    //println!("DEBUG: ver_old={} ver_new={}", old_val.version, new_val.version);

    if new_val.version & 1 != 0 {
      //continue
    } else if new_val.version != (old_val.version & !1) {
      //continue
    } else {
      break;
    }
  }
  ret
}

//todo: move to generic
pub struct CalendarTime {
  pub year: u16,
  pub month: u8,
  pub day: u8,
  pub hour: u8,
  pub min: u8,
  pub sec: u8
}

fn get_calendar_from_epoch(sec: u64) -> CalendarTime {
  const DAYS_OF_MONTH: [u64; 12] = [0,31,61,92,122,153,184,214,245,275,306,337];
  const DAYS_OF_ONE_YEAR: u64 = 365;
  const DAYS_OF_FOUR_YEAR: u64 = DAYS_OF_ONE_YEAR * 4 + 1;
  const DAYS_OF_HUNDRED_YEAR: u64 = DAYS_OF_FOUR_YEAR * 25 - 1;
  const DAYS_OF_FOUR_HUNDRED_YEAR: u64 = DAYS_OF_HUNDRED_YEAR * 4 + 1;

  let second = sec % 60;
  let minute = (sec / 60) % 60;
  let hour = (sec / 3600) % 24;

  let day_from_epoch = sec / (24*60*60);
  let weekday = (day_from_epoch + 3) % 7;
  let day_from_march = day_from_epoch + (1969*365 + 1969/4 - 1969/100 + 1969/400 + 306); //1969years, 306days

  let mut year = 400 * (day_from_march / DAYS_OF_FOUR_HUNDRED_YEAR);
  let month;
  let mut day = day_from_march % DAYS_OF_FOUR_HUNDRED_YEAR;

  let n = day / DAYS_OF_HUNDRED_YEAR;
  year = year + n * 100;
  day = day % DAYS_OF_HUNDRED_YEAR;

  let leap;
  if n == 4 {
    leap = true;
  } else {
    year = year + (day / DAYS_OF_FOUR_YEAR) * 4;
    day = day % DAYS_OF_FOUR_YEAR;

    let n = day / DAYS_OF_ONE_YEAR;
    year = year + n;
    day = day % DAYS_OF_ONE_YEAR;
    if n == 4 {
      leap = true;
    } else {
      leap = false;
    }
  }

  if leap {
    month = 2;
    day = 29;
  } else {
    let month_diff = (day * 5 + 2) / 153;
    day = day - DAYS_OF_MONTH[month_diff as usize] + 1;
    if month_diff > 9 {
      // Jan,Feb
      year = year + 1;
      month = month_diff - 9;
    } else {
      month = month_diff + 3;
    }
  }

  CalendarTime {
    year: year as u16,
    month: month as u8,
    day: day as u8,
    hour: hour as u8,
    min: minute as u8,
    sec: second as u8,
  }
}

pub fn get_calendar() -> CalendarTime {
  let mut wc_sec;
  let mut wc_nsec;

  loop {
    let old_val = unsafe { read_volatile(&KVM_CLOCK_WALL_CLOCK as *const PVClockWallClock) };
    fence(Ordering::SeqCst);
    wc_sec = old_val.sec as u64;
    wc_nsec = old_val.nsec as u64;
    fence(Ordering::SeqCst);
    let new_val = unsafe { read_volatile(&KVM_CLOCK_WALL_CLOCK as *const PVClockWallClock) };
    if new_val.version & 1 != 0 {
      //continue
    } else if new_val.version != old_val.version {
      //continue
    } else {
      break;
    }
  }


  const SEC_TO_NSEC: u64 = 1_000_000_000;
  let delta = get_monotonic_time();
  let nsec_from_epoch = wc_sec * SEC_TO_NSEC + wc_nsec + get_monotonic_time();
  let sec_from_epoch = nsec_from_epoch / SEC_TO_NSEC;

  //println!("DEBUG: UNIXTIME : {}", sec_from_epoch);

  get_calendar_from_epoch(sec_from_epoch)
}