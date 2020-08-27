use alloc::collections::BTreeMap;
use alloc::sync::Arc;

use crate::spinlock::{ Spinlock, const_spinlock };
use crate::devices::netif::Netif;
use crate::net::ethernet::MacAddress;
use crate::net::ipv4::Ipv4Address;
use crate::net::ipv6::Ipv6Address;

#[derive(Copy, Clone)]
pub enum FIBType {
  Remote,
  Adjacent,
  AdjacentResolved,
  Local,
}

#[derive(Clone)]
pub struct ForwardInformationBaseIpv4 {
  nexthop_macaddress: MacAddress,
  nexthop_address: Ipv4Address,
  netif: Arc<dyn Netif>,
  fib_type: FIBType,
}

impl ForwardInformationBaseIpv4 {
  pub fn new(nexthop_macaddress: MacAddress, nexthop_address: Ipv4Address, netif: Arc<dyn Netif>, fib_type: FIBType) -> ForwardInformationBaseIpv4 {
    ForwardInformationBaseIpv4 {
      nexthop_macaddress: nexthop_macaddress,
      nexthop_address: nexthop_address,
      netif: netif,
      fib_type: fib_type, 
    }
  }

  pub fn get_nexthop_macaddress(&self) -> MacAddress {
    self.nexthop_macaddress
  }

  pub fn get_nexthop_address(&self) -> Ipv4Address {
    self.nexthop_address
  }

  pub fn get_netif(&self) -> &Arc<dyn Netif> {
    &self.netif
  }

  pub fn get_fib_type(&self) -> FIBType {
    self.fib_type
  }
}

pub static mut IPV4_FIB_INDEX: [BTreeMap<Ipv4Address, ForwardInformationBaseIpv4>; 33] = [
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), 
];

fn ipv4_mask_to_prefixlen(mask: u32) -> usize {
  match mask {
    0xffffffff => 32,
    0xfffffffe => 31,
    0xfffffffc => 30,
    0xfffffff8 => 29,
    0xfffffff0 => 28,
    0xffffffe0 => 27,
    0xffffffc0 => 26,
    0xffffff80 => 25,
    0xffffff00 => 24,
    0xfffffe00 => 23,
    0xfffffc00 => 22,
    0xfffff800 => 21,
    0xfffff000 => 20,
    0xffffe000 => 19,
    0xffffc000 => 18,
    0xffff8000 => 17,
    0xffff0000 => 16,
    0xfffe0000 => 15,
    0xfffc0000 => 14,
    0xfff80000 => 13,
    0xfff00000 => 12,
    0xffe00000 => 11,
    0xffc00000 => 10,
    0xff800000 => 9,
    0xff000000 => 8,
    0xfe000000 => 7,
    0xfc000000 => 6,
    0xf8000000 => 5,
    0xf0000000 => 4,
    0xe0000000 => 3,
    0xc0000000 => 2,
    0x80000000 => 1,
    0x00000000 => 0,
    _ => 0,
  }
}

pub fn register_ipv4_fib(ip_address: Ipv4Address, mask: u32, nexthop_macaddress: MacAddress, nexthop_address: Ipv4Address, netif: Arc<dyn Netif>, fib_type: FIBType) {
  let fib_index = ipv4_mask_to_prefixlen(mask);

  let table = unsafe { &mut IPV4_FIB_INDEX[fib_index] };
  table.insert(ip_address, ForwardInformationBaseIpv4::new(nexthop_macaddress, nexthop_address, netif, fib_type));
}

pub fn find_ipv4_fib(ip_address: &Ipv4Address, mask: u32) -> Option<&'static ForwardInformationBaseIpv4> {
  let mut fib_index = ipv4_mask_to_prefixlen(mask);

  // longest match
  loop {
    if let Some(fib) = unsafe { IPV4_FIB_INDEX[fib_index].get(&ip_address.masked(fib_index as u32)) } {
      return Some(fib)
    }

    if fib_index == 0 {
      break;
    } 

    fib_index = fib_index - 1;
  }
  None
}

#[derive(Clone)]
pub struct ForwardInformationBaseIpv6 {
  nexthop_macaddress: MacAddress,
  nexthop_address: Ipv6Address,
  netif: Arc<dyn Netif>,
  fib_type: FIBType,
}

impl ForwardInformationBaseIpv6 {
  pub fn new(nexthop_macaddress: MacAddress, nexthop_address: Ipv6Address, netif: Arc<dyn Netif>, fib_type: FIBType) -> ForwardInformationBaseIpv6 {
    ForwardInformationBaseIpv6 {
      nexthop_macaddress: nexthop_macaddress,
      nexthop_address: nexthop_address,
      netif: netif,
      fib_type: fib_type, 
    }
  }

  pub fn get_nexthop_macaddress(&self) -> MacAddress {
    self.nexthop_macaddress
  }

  pub fn get_nexthop_address(&self) -> Ipv6Address {
    self.nexthop_address
  }

  pub fn get_netif(&self) -> &Arc<dyn Netif> {
    &self.netif
  }

  pub fn get_fib_type(&self) -> FIBType {
    self.fib_type
  }
}


pub static mut IPV6_FIB_INDEX: [BTreeMap<Ipv6Address, ForwardInformationBaseIpv6>; 129] = [
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new(),
  BTreeMap::new(), 
];

pub fn register_ipv6_fib(ip_address: Ipv6Address, prefix: u32, nexthop_macaddress: MacAddress, nexthop_address: Ipv6Address, netif: Arc<dyn Netif>, fib_type: FIBType) {
  let table = unsafe { &mut IPV6_FIB_INDEX[prefix as usize] };
  table.insert(ip_address, ForwardInformationBaseIpv6::new(nexthop_macaddress, nexthop_address, netif, fib_type));
}

pub fn find_ipv6_fib(ip_address: &Ipv6Address, prefix: u32) -> Option<&'static ForwardInformationBaseIpv6> {
  let mut fib_index = prefix as usize;

  // longest match
  loop {
    if let Some(fib) = unsafe { IPV6_FIB_INDEX[fib_index].get(&ip_address.masked(prefix)) } {
      return Some(fib)
    }

    if fib_index == 0 {
      break;
    } 

    fib_index = fib_index - 1;
  }
  None
}


#[derive(Clone)]
pub struct AdjacentInformation {
  mac_address: MacAddress, 
  netif: Arc<dyn Netif>,
  is_local: bool,
  expire_time: Option<u64>, //permanent entry if expire_time is None
}

impl AdjacentInformation {
  pub const fn new(mac_address: MacAddress, netif: Arc<dyn Netif>, is_local: bool, expire_time: Option<u64>) -> AdjacentInformation {
    AdjacentInformation {
      mac_address: mac_address,
      netif: netif,
      is_local: is_local,
      expire_time: expire_time,
    }
  }

  pub fn get_netif(&self) -> &Arc<dyn Netif> {
    &self.netif
  }
  pub fn is_local(&self) -> bool {
    self.is_local
  }
  pub fn get_expire_time(&self) -> Option<u64> {
    self.expire_time
  }
}

pub static IPV4_ADJACENT: Spinlock<BTreeMap<Ipv4Address, AdjacentInformation>> = const_spinlock(BTreeMap::new());

pub fn register_ipv4_adjacent(ip_address: Ipv4Address, mac_address: MacAddress, netif: Arc<dyn Netif>, is_local: bool, expire_time: Option<u64>) {
  //record to adj-table
  let mut adj_table = IPV4_ADJACENT.lock();
  if let Some(adj) = adj_table.get(&ip_address) {
    if let Some(expire_time_of_existing) = adj.get_expire_time() {
      //todo: check lifetime and register this mac if expired.
      adj_table.insert(ip_address, AdjacentInformation::new(mac_address, netif, is_local, expire_time));
    } else {
      // do nothing if the existing entry is permanent
    }
  } else {
    // register this mac
    adj_table.insert(ip_address, AdjacentInformation::new(mac_address, netif, is_local, expire_time));
  }
}

pub static IPV6_ADJACENT: Spinlock<BTreeMap<Ipv6Address, AdjacentInformation>> = const_spinlock(BTreeMap::new());

pub fn register_ipv6_adjacent(ip_address: Ipv6Address, mac_address: MacAddress, netif: Arc<dyn Netif>, is_local: bool, expire_time: Option<u64>) {
  //record to adj-table
  let mut adj_table = IPV6_ADJACENT.lock();
  if let Some(adj) = adj_table.get(&ip_address) {
    if let Some(expire_time_of_existing) = adj.get_expire_time() {
      //todo: check lifetime and register this mac if expired.
      adj_table.insert(ip_address, AdjacentInformation::new(mac_address, netif, is_local, expire_time));
    } else {
      // do nothing if the existing entry is permanent
    }
  } else {
    // register this mac
    adj_table.insert(ip_address, AdjacentInformation::new(mac_address, netif, is_local, expire_time));
  }
}


pub static MAC_ADDR_TABLE: Spinlock<BTreeMap<MacAddress, AdjacentInformation>> = const_spinlock(BTreeMap::new());

pub fn register_macaddress(mac_address: MacAddress, netif: Arc<dyn Netif>, is_local: bool, expire_time: Option<u64>) {
  //record to adj-table
  let mut mactable = MAC_ADDR_TABLE.lock();
  if let Some(adj) = mactable.get(&mac_address) {
    if let Some(expire_time_of_existing) = adj.get_expire_time() {
      //todo: check lifetime and register this mac if expired.
      mactable.insert(mac_address, AdjacentInformation::new(mac_address, netif, is_local, expire_time));
    } else {
      // do nothing if the existing entry is permanent
    }
  } else {
    // register this mac
    mactable.insert(mac_address, AdjacentInformation::new(mac_address, netif, is_local, expire_time));
  }
}