use core::fmt;
use crate::devices::serial;

pub static mut CONSOLE: Option<serial::SerialPort> = None;

#[macro_export]
macro_rules! println {
  ($fmt:expr) => (print!(concat!($fmt, "\n")));
  ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}

#[macro_export]
macro_rules! print {
  ($($arg:tt)*) => ({
    crate::console::print(format_args!($($arg)*));
  });
}

pub fn init() {
  // setup console
  // todo: support console other than serial
  unsafe {
    CONSOLE = Some(
      serial::SerialPort::new(serial::COM1PORT)
    );
  }
}

pub fn print(args: fmt::Arguments) {
  use core::fmt::Write;
  unsafe {
    if let Some(mut console) = CONSOLE {
      console.write_fmt(args).unwrap();
    }
  }
}
