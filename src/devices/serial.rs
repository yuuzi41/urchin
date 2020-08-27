// UART 16550A Compatible Serial port driver

pub const COM1PORT: u16 = 0x03f8;

use crate::io::outb;
use crate::io::inb;

use core::fmt;

#[derive(Debug, Copy, Clone)]
pub struct SerialPort {
  port: u16
}

impl SerialPort {
  pub fn new(port: u16) -> SerialPort {
    init_serial(port);
    SerialPort { port: port }
  }

  pub fn read_byte(&self) -> u8 {
    loop {
      if is_received(self.port) == true {
        unsafe {
          return inb(self.port);
        }
      }
    }
  }

  pub fn write_byte(&self, val: u8) {
    loop {
      if is_transmit_empty(self.port) == true {
        unsafe {
          outb(self.port, val);
        }
        break;
      }
    }
  } 
}

impl fmt::Write for SerialPort {
  fn write_str(&mut self, s: &str) -> fmt::Result {
    let bytes = s.as_bytes();
    for (_i, &item) in bytes.iter().enumerate() {
      self.write_byte(item);
    }
    Ok(())
  }
}

fn init_serial(port: u16) {
  unsafe {
    outb(port + 1, 0x00);    // Disable all interrupts
    outb(port + 3, 0x80);    // Enable DLAB (set baud rate divisor)
    outb(port + 0, 0x03);    // Set divisor to 3 (lo byte) 38400 baud
    outb(port + 1, 0x00);    //                  (hi byte)
    outb(port + 3, 0x03);    // 8 bits, no parity, one stop bit
    outb(port + 2, 0xC7);    // Enable FIFO, clear them, with 14-byte threshold
    outb(port + 4, 0x0B);    // IRQs enabled, RTS/DSR set
  }
}

fn is_received(port: u16) -> bool {
  unsafe {
    (inb(port + 5) & 0x01) == 1
  }
}

fn is_transmit_empty(port: u16) -> bool {
  unsafe {
    (inb(port + 5) & 0x20) == 0x20
  }
}
