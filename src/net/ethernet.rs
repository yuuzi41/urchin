use core::cmp::Ordering;

use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::devices::buffer::Buffer;
use crate::net::{DataFromNetif, ProcessingNode};
use crate::net::fib::{MAC_ADDR_TABLE, AdjacentInformation,register_macaddress};
use crate::net::arp::ArpIn;
use crate::PROC_NODES;

#[derive(Debug, Copy, Clone)]
pub struct MacAddress {
  addr: [u8; 6],
  addr_prim: u64,
}

impl MacAddress {
  pub fn new(addr: [u8; 6]) -> MacAddress {
    let addr_prim = 
    (addr[5] as u64) << 40 |
    (addr[4] as u64) << 32 |
    (addr[3] as u64) << 24 | 
    (addr[2] as u64) << 16 |
    (addr[1] as u64) << 8 |
    (addr[0] as u64);
    
    MacAddress { addr: addr, addr_prim: addr_prim }
  }

  pub fn get_array(&self) -> [u8; 6] {
    self.addr
  }

  pub fn get_prim(&self) -> u64 {
    self.addr_prim
  }
}

impl Ord for MacAddress {
  fn cmp(&self, other: &Self) -> Ordering {
      self.addr_prim.cmp(&other.addr_prim)
  }
}

impl PartialOrd for MacAddress {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
      Some(self.cmp(other))
  }
}

impl Eq for MacAddress {}

impl PartialEq for MacAddress {
  fn eq(&self, other: &Self) -> bool {
    self.addr_prim == other.addr_prim
  }
}

////////

pub struct EthernetIn;

impl EthernetIn {
  pub const fn new() -> EthernetIn {
    EthernetIn {}
  }
}

#[repr(C,packed)]
struct EthernetFrame {
  dest_addr: [u8; 6],
  src_addr: [u8; 6],
  frame_type: [u8; 2],
}

pub fn generate_ether_header(buffer: &mut [u8], src_addr: MacAddress, dest_addr: MacAddress, frame_type: [u8; 2]) {
  let mut header = unsafe { &mut *((&mut buffer[0] as *mut _) as *mut EthernetFrame) };
  header.dest_addr = dest_addr.get_array();
  header.src_addr = src_addr.get_array();
  header.frame_type = frame_type;
}

impl ProcessingNode for EthernetIn {
  fn process(&self, buff: &[DataFromNetif]) {
    let mut arp_pkts = Vec::with_capacity(buff.len());
    let mut ipv4_pkts = Vec::with_capacity(buff.len());
    let mut ipv6_pkts = Vec::with_capacity(buff.len());

    for frame in buff.iter() {
      let slice = frame.get_buffer().slice();
      let header = unsafe { &*((&slice[0] as *const _) as *const EthernetFrame) };
      let mut proc_frame = || {
        match header.frame_type {
          [0x08, 0x06] => {
            //ARP
            arp_pkts.push(frame.clone());
          },
          [0x08, 0x00] => {
            //IPv4
            ipv4_pkts.push(frame.clone());
          },
          [0x81, 0x00] => {
            //VLAN
          },
          [0x86, 0xDD] => {
            //IPv6
            ipv6_pkts.push(frame.clone());
          },
          _ => { /* unknown */ },
        }
      };

      // learn MAC address
      register_macaddress(MacAddress::new(header.src_addr), Arc::clone(frame.get_netif()), false, None);      

      let mactable = MAC_ADDR_TABLE.lock();
      if header.dest_addr == [0xff, 0xff, 0xff, 0xff, 0xff, 0xff] {
        //broadcast address
        proc_frame();
        //todo: l2sw with flood
        //todo: multicast mac
      } else if let Some(adj) = mactable.get(&MacAddress::new(header.dest_addr)) {
        if adj.is_local() {
          // this mac address is myself. so I'll process it.
          proc_frame();

          // even if it's own, i must do switching l2 if it's multicast.
          match header.dest_addr[0..3] {
            [0x33, 0x33, _] => (), //IPv6 Multicast
            [0x01, 0x00, 0x5e] => (), //IPv4 Multicast
            _ => (),
          }
        } else {
          // i know this mac address but it's not own.
          // todo: implement l2 switching
        }
      } else {
        // i don't know this mac address.
        // todo: implement l2 switching with flooding
      }
    }

    if arp_pkts.len() > 0 {
      if let Some(node_ref) = unsafe { PROC_NODES.get("arp-in") } {
        node_ref.process(&arp_pkts);
      }
    }
    if ipv4_pkts.len() > 0 {
      if let Some(node_ref) = unsafe { PROC_NODES.get("ipv4-in") } {
        node_ref.process(&ipv4_pkts);
      }
    }
    if ipv6_pkts.len() > 0 {
      if let Some(node_ref) = unsafe { PROC_NODES.get("ipv6-in") } {
        node_ref.process(&ipv6_pkts);
      }
    }
  }
}
