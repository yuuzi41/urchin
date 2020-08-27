pub mod ethernet;
pub mod arp;
pub mod ipv4;
pub mod ipv6;
pub mod fib;

use core::future::Future;

use alloc::sync::Arc;

use crate::devices::netif::Netif;
use crate::devices::buffer::Buffer;

#[derive(Clone)]
pub struct DataFromNetif {
  netif: Arc<Netif>,
  buffer: Arc<Buffer>,
}

impl DataFromNetif {
  pub const fn new(netif: Arc<Netif>, buffer: Arc<Buffer>) -> DataFromNetif {
    DataFromNetif {
      netif: netif,
      buffer: buffer,
    }
  }

  pub fn get_netif(&self) -> &Arc<Netif> {
    &self.netif
  }

  pub fn get_buffer(&self) -> &Arc<Buffer> {
    &self.buffer
  }
}

/////////

pub trait ProcessingNode {
  fn process(&self, buff: &[DataFromNetif]);
  fn proc(&self) -> impl Future<Output = ()>;
}
