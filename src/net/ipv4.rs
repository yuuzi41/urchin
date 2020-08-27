use core::convert::TryInto;
use core::cmp::Ordering;

use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::devices::buffer::Buffer;
use crate::net::{DataFromNetif, ProcessingNode};
use crate::net::ethernet::{MacAddress, generate_ether_header};
use crate::net::fib::{FIBType, MAC_ADDR_TABLE, AdjacentInformation, find_ipv4_fib, register_ipv4_fib, register_ipv4_adjacent, IPV4_ADJACENT};
use crate::PROC_NODES;

#[derive(Debug, Copy, Clone)]
pub struct Ipv4Address {
  addr_prim: u32,
}

impl Ipv4Address {
  pub fn from_array(arr: [u8; 4]) -> Ipv4Address {
    let prim = (arr[0] as u32) << 24 | (arr[1] as u32) << 16 | (arr[2] as u32) << 8 | (arr[3] as u32);
    Ipv4Address { addr_prim: prim }
  }

  pub fn masked(&self, prefix_length: u32) -> Ipv4Address {
    let mask = 0xffffffffu32 << (32 - prefix_length);
    Ipv4Address { addr_prim: self.addr_prim & mask }
  }
        
  pub fn get_array(&self) -> [u8; 4] {
    [(self.addr_prim >> 24) as u8, (self.addr_prim >> 16) as u8, (self.addr_prim >> 8) as u8, self.addr_prim as u8]
  }

  pub fn get_prim(&self) -> u32 {
    self.addr_prim
  }
}

impl Ord for Ipv4Address {
  fn cmp(&self, other: &Self) -> Ordering {
      self.addr_prim.cmp(&other.addr_prim)
  }
}

impl PartialOrd for Ipv4Address {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
      Some(self.cmp(other))
  }
}

impl Eq for Ipv4Address {}

impl PartialEq for Ipv4Address {
  fn eq(&self, other: &Self) -> bool {
    self.addr_prim == other.addr_prim
  }
}

////

pub struct Ipv4In;

impl Ipv4In {
  pub const fn new() -> Ipv4In {
    Ipv4In {}
  }
}

#[repr(C,packed)]
struct Ipv4Packet {
  version_and_ihl: u8,
  tos: u8,
  length: [u8; 2],
  id: u16,
  flag_and_frag: u16,
  ttl: u8,
  proto: u8,
  checksum: u16,
  src_ip: [u8; 4],
  dest_ip: [u8; 4],
}

pub fn generate_ipv4_header(buffer: &mut [u8], len: [u8;2], proto: u8, src_ip: Ipv4Address, dest_ip: Ipv4Address) {
  let mut ipv4_hdr = unsafe { &mut *((&mut buffer[0] as *mut _) as *mut Ipv4Packet) };
  ipv4_hdr.version_and_ihl = 0x45;
  ipv4_hdr.tos = 0;
  ipv4_hdr.length = len;
  ipv4_hdr.id = 0;
  ipv4_hdr.flag_and_frag = 0x0040;
  ipv4_hdr.ttl = 64;
  ipv4_hdr.proto = proto;
  ipv4_hdr.checksum = 0;
  ipv4_hdr.src_ip = src_ip.get_array();
  ipv4_hdr.dest_ip = dest_ip.get_array();
}

impl ProcessingNode for Ipv4In {
  fn process(&self, buff: &[DataFromNetif]) {
    let mut icmp_pkts = Vec::with_capacity(buff.len());

    for frame in buff.iter() {
      let slice = frame.get_buffer().slice();
      let ipv4_hdr = unsafe { &*((&slice[14] as *const _) as *const Ipv4Packet) };
      let dest_ip_addr = Ipv4Address::from_array(ipv4_hdr.dest_ip);

      if (ipv4_hdr.version_and_ihl & 0xf0) != 0x40 {
        // irregular ip packet. drop it.
        continue;
      }

      if let Some(fib) = find_ipv4_fib(&dest_ip_addr, 0xffffffff) {
        //println!("fib is found. {:?} mac={:?}", fib.get_nexthop_address().get_array(), fib.get_nexthop_macaddress().get_array());
        match fib.get_fib_type() {
          FIBType::Local => {
            match ipv4_hdr.proto {
              0x01 => icmp_pkts.push(frame.clone()), //ICMP
              _ => (),
            }
          },
          FIBType::Adjacent => {
            // must resolve mac address using arp.
          },
          FIBType::AdjacentResolved => {
            //forward
          },
          FIBType::Remote => {
            //forward
          },
        }
      } else {
        // fib not found. cannot handle this packet.
      }

      if icmp_pkts.len() > 0 {
        if let Some(node_ref) = unsafe { PROC_NODES.get("icmpv4-in-local") } {
          node_ref.process(&icmp_pkts);
        }
      }
    }
  }
}

///////

pub struct Icmpv4InLocal;

impl Icmpv4InLocal {
  pub const fn new() -> Icmpv4InLocal {
    Icmpv4InLocal {}
  }
}

#[repr(C,packed)]
struct Icmpv4Packet {
  icmp_type: u8,
  code: u8,
  checksum: u16,
  identifier: u16,
  sequence: u16,
}

impl ProcessingNode for Icmpv4InLocal {
  fn process(&self, buff: &[DataFromNetif]) {
    for frame in buff.iter() {
      let slice = frame.get_buffer().slice();
      let ipv4_hdr = unsafe { &*((&slice[14] as *const _) as *const Ipv4Packet) };
      let icmpv4_hdr = unsafe { &*((&slice[14+((ipv4_hdr.version_and_ihl & 0x0f) as usize * 4)] as *const _) as *const Icmpv4Packet) };
      let dest_ip_addr = Ipv4Address::from_array(ipv4_hdr.dest_ip);

      match icmpv4_hdr.icmp_type {
        0x08 => {
          //echo request
          //println!("Got ICMP Echo request");

          let length = ((ipv4_hdr.length[0] as usize) << 8 | ipv4_hdr.length[1] as usize) - ((ipv4_hdr.version_and_ihl & 0x0f) as usize * 4);
          let netif = Arc::clone(frame.get_netif());
          let respbuff = netif.pre_xmit(14+20+length);
          let respslice = respbuff.slice_mut();

          //println!("length = {}", length);

          let dest_mac = MacAddress::new(slice[6..12].try_into().unwrap());
          generate_ether_header(&mut respslice[0..], *netif.get_macaddress(), dest_mac, [0x08, 0x00]);
          generate_ipv4_header(&mut respslice[14..], ipv4_hdr.length, 0x01, Ipv4Address::from_array(ipv4_hdr.dest_ip), Ipv4Address::from_array(ipv4_hdr.src_ip));

          let mut icmpv4_send_hdr = unsafe { &mut *((&mut respslice[14+20] as *mut _) as *mut Icmpv4Packet) };
          icmpv4_send_hdr.icmp_type = 0x00; //icmp echo reply
          icmpv4_send_hdr.code = 0x00;
          icmpv4_send_hdr.checksum = 0;
          icmpv4_send_hdr.identifier = icmpv4_hdr.identifier;
          icmpv4_send_hdr.sequence = icmpv4_hdr.sequence;
          for i in (14+20+8)..(14+20+length) {
            respslice[i] = slice[i];
          }

          let mut csum_icmp: u32 = 0;
          for i in ((14+20)/2)..((14+20+length)/2) {
            csum_icmp = csum_icmp + ((respslice[i*2] as u32) << 8 | (respslice[i*2+1] as u32))
          }
          csum_icmp = (csum_icmp & 0x0000ffff) + (csum_icmp >> 16);
          csum_icmp = (csum_icmp & 0x0000ffff) + (csum_icmp >> 16);
          csum_icmp = !csum_icmp;
          icmpv4_send_hdr.checksum = ((csum_icmp & 0x00ff) << 8 | (csum_icmp & 0xff00) >> 8) as u16;

          let mut csum_ipv4: u32 = 0;
          for i in (14/2)..((14+20+length)/2) {
            csum_ipv4 = csum_ipv4 + ((respslice[i*2] as u32) << 8 | (respslice[i*2+1] as u32))
          }
          csum_ipv4 = (csum_ipv4 & 0x0000ffff) + (csum_ipv4 >> 16);
          csum_ipv4 = (csum_ipv4 & 0x0000ffff) + (csum_ipv4 >> 16);
          csum_ipv4 = !csum_ipv4;
          respslice[24] = (csum_ipv4 >> 8) as u8;
          respslice[25] = (csum_ipv4) as u8;
          
          netif.xmit(respbuff);
        },
        _ => (),
      }
    }
  }
}
