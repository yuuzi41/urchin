use alloc::sync::Arc;

use crate::devices::buffer;
use crate::net;

pub enum Error {
  TransmitError(),
  ReceiveError(),
}

pub trait Netif : Sync + Send {
  fn pre_xmit(&self, size: usize) -> Arc<buffer::Buffer>;
  fn xmit(&self, buffer: Arc<buffer::Buffer>) -> Result<(), Error>;
  fn recv(&self);

  fn get_id(&self) -> usize;
  fn get_macaddress(&self) -> &net::ethernet::MacAddress;
  fn get_drivername(&self) -> &'static str;
}
