use alloc::sync::Arc;
use crate::arch::x86_64::io;
use crate::arch::x86_64::apic;

/*

pub struct InterruptMaskGuard<'a T> {
  mask: &'a InterruptMask<T>,
}

impl<'a T> InterruptMaskGuard<'a T> {
  fn new(mask: InterruptMask<T>) {
    InterruptMaskGuard<'a T> { mask: mask }
  }
}

impl<'a, T: ?Sized + 'a> Deref for InterruptMaskGuard<'a, R, T> {
  type Target = T;
  #[inline]
  fn deref(&self) -> &T {
    unsafe { &*self.mutex.data.get() }
  }
}

impl<'a, T: ?Sized + 'a> DerefMut for InterruptMaskGuard<'a, R, T> {
  #[inline]
  fn deref_mut(&mut self) -> &mut T {
      unsafe { &mut *self.mutex.data.get() }
  }
}

impl<'a, T: ?Sized + 'a> Drop for InterruptMaskGuard<'a, T> {
  #[inline]
  fn drop(&mut self) {
      // Safety: A MutexGuard always holds the lock.
      unsafe {
          self.mutex.raw.unlock();
      }
  }
}

pub struct InterruptMask<T> {
  inner: T,
}

impl<T> InterruptMask<T> {
  pub fn new(inner: T) {
    InterruptMask<T> { inner: inner }
  }
}
*/

pub trait Interruptable {
  fn get_irq(&self) -> u8;
  fn interrupt_handler(&self);
}

pub struct IrqHandler {
  handlers: [Option<Arc<dyn Interruptable>>; 256],
}

impl<'a> IrqHandler {
  pub const fn new() -> IrqHandler {
    // it seems not so clever.
    IrqHandler { handlers: [
      None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,  
      None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,  
      None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,  
      None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,  
      None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,  
      None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,  
      None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,  
      None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,  
      None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,  
      None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,  
      None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,  
      None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,  
      None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,  
      None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,  
      None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,  
      None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,  
    ] }
  }

  pub fn set_handler(&mut self, handler: Arc<dyn Interruptable>) {
    let irq = handler.get_irq();
    self.handlers[irq as usize] = Some(handler);
    apic::enable_irq(irq);
  }
  
  pub fn unset_handler(&mut self, irq: u8) {
    self.handlers[irq as usize] = None;
    apic::disable_irq(irq);
  }

  fn get_handler(&self, irq: u8) -> Option<&Arc<dyn Interruptable>> {
    match &self.handlers[irq as usize] {
      Some(arc) => Some(&arc),
      None => None,
    }
  }
}

pub static mut IRQ_HANDLERS: IrqHandler = IrqHandler::new();

pub fn irq_handler(irq: u8) {
  apic::ack_irq();

  disable_interrupt();
  
  let handler_opt = unsafe { IRQ_HANDLERS.get_handler(irq) };
  if let Some(handle) = handler_opt {
    handle.interrupt_handler();
  } else {
    //println!("DEBUG: IRQ {} Interrupted but no handler.", irq);
  }
}

pub fn enable_interrupt() {
  unsafe {
    io::enable_interrupt();
  }
}

pub fn disable_interrupt() {
  unsafe {
    io::disable_interrupt();
  }
}


////////////

pub struct Timer {}

impl Timer {
  pub fn new() -> Self {
    Timer {}
  }
}

impl Interruptable for Timer{
  fn get_irq(&self) -> u8 {
    0
  }

  // assume that this is called for each 1ms
  fn interrupt_handler(&self) {
    use crate::asynchronous::timer::check_timerfutures_from_interrupt;

    check_timerfutures_from_interrupt();
  }
}