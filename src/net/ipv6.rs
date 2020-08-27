use core::convert::TryInto;
use core::cmp::Ordering;

use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::devices::buffer::Buffer;
use crate::net::{DataFromNetif, ProcessingNode};
use crate::net::ethernet::{MacAddress, generate_ether_header};
use crate::net::fib::{FIBType, MAC_ADDR_TABLE, AdjacentInformation, register_macaddress, register_ipv6_adjacent, register_ipv6_fib, find_ipv6_fib};
use crate::PROC_NODES;

#[derive(Debug, Copy, Clone)]
pub struct Ipv6Address {
  addr_prim: u128,
}

impl Ipv6Address {
  pub fn from_array(arr: [u8; 16]) -> Ipv6Address {
    let prim = 
    (arr[0] as u128) << 120 | 
    (arr[1] as u128) << 112 | 
    (arr[2] as u128) << 104 | 
    (arr[3] as u128) << 96 | 
    (arr[4] as u128) << 88 | 
    (arr[5] as u128) << 80 | 
    (arr[6] as u128) << 72 | 
    (arr[7] as u128) << 64 | 
    (arr[8] as u128) << 56 | 
    (arr[9] as u128) << 48 | 
    (arr[10] as u128) << 40 | 
    (arr[11] as u128) << 32 | 
    (arr[12] as u128) << 24 | 
    (arr[13] as u128) << 16 | 
    (arr[14] as u128) << 8 | 
    (arr[15] as u128);
    Ipv6Address { addr_prim: prim }
  }

  pub fn masked(&self, prefix_length: u32) -> Ipv6Address {
    let mask = 0xffffffff_ffffffff_ffffffff_ffffffffu128 << (128 - prefix_length);
    Ipv6Address { addr_prim: self.addr_prim & mask }
  }
        
  pub fn get_array(&self) -> [u8; 16] {
    [
      (self.addr_prim >> 120) as u8,
      (self.addr_prim >> 112) as u8,
      (self.addr_prim >> 104) as u8,
      (self.addr_prim >> 96) as u8,
      (self.addr_prim >> 88) as u8,
      (self.addr_prim >> 80) as u8,
      (self.addr_prim >> 72) as u8,
      (self.addr_prim >> 64) as u8,
      (self.addr_prim >> 56) as u8,
      (self.addr_prim >> 48) as u8,
      (self.addr_prim >> 40) as u8,
      (self.addr_prim >> 32) as u8,
      (self.addr_prim >> 24) as u8,
      (self.addr_prim >> 16) as u8,
      (self.addr_prim >> 8) as u8,
      (self.addr_prim) as u8,
    ]
  }

  pub fn get_prim(&self) -> u128 {
    self.addr_prim
  }
}

impl Ord for Ipv6Address {
  fn cmp(&self, other: &Self) -> Ordering {
      self.addr_prim.cmp(&other.addr_prim)
  }
}

impl PartialOrd for Ipv6Address {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
      Some(self.cmp(other))
  }
}

impl Eq for Ipv6Address {}

impl PartialEq for Ipv6Address {
  fn eq(&self, other: &Self) -> bool {
    self.addr_prim == other.addr_prim
  }
}

/////////

pub struct Ipv6In;

impl Ipv6In {
  pub const fn new() -> Ipv6In {
    Ipv6In {}
  }
}

#[repr(C,packed)]
struct Ipv6Packet {
  version_and_tch: u8,
  tcl_and_flh: u8,
  flm: u8,
  fll: u8,
  length: [u8; 2],
  nexthdr: u8,
  hoplimit: u8,
  src_ip: [u8; 16],
  dest_ip: [u8; 16],
}

pub fn generate_ipv6_header(buffer: &mut [u8], len: [u8;2], nexthdr: u8, src_ip: Ipv6Address, dest_ip: Ipv6Address) {
  let mut ipv6_hdr = unsafe { &mut *((&mut buffer[0] as *mut _) as *mut Ipv6Packet) };
  ipv6_hdr.version_and_tch = 0x60;
  ipv6_hdr.tcl_and_flh = 0;
  ipv6_hdr.flm = 0;
  ipv6_hdr.fll = 0;
  ipv6_hdr.length = len;
  ipv6_hdr.nexthdr = nexthdr;
  ipv6_hdr.hoplimit = 0x80;
  ipv6_hdr.src_ip = src_ip.get_array();
  ipv6_hdr.dest_ip = dest_ip.get_array();
}

impl ProcessingNode for Ipv6In {
  fn process(&self, buff: &[DataFromNetif]) {
    let mut icmp_pkts = Vec::with_capacity(buff.len());

    for frame in buff.iter() {
      let slice = frame.get_buffer().slice();
      let ipv6_hdr = unsafe { &*((&slice[14] as *const _) as *const Ipv6Packet) };
      let dest_ip_addr = Ipv6Address::from_array(ipv6_hdr.dest_ip);

      if ipv6_hdr.version_and_tch & 0xf0 != 0x60 {
        // irregular ip packet. drop it.
        continue;
      }

      if let Some(fib) = find_ipv6_fib(&dest_ip_addr, 128) {
        //println!("fib is found. {:?} mac={:?}", fib.get_nexthop_address().get_array(), fib.get_nexthop_macaddress().get_array());
        match fib.get_fib_type() {
          FIBType::Local => {
            match ipv6_hdr.nexthdr {
              58 => icmp_pkts.push(frame.clone()), //ICMP
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
        if let Some(node_ref) = unsafe { PROC_NODES.get("icmpv6-in-local") } {
          node_ref.process(&icmp_pkts);
        }
      }
      //println!("IPv6 payload={} nexthdr={}", (ipv6_hdr.length[0] as u16) << 8 | (ipv6_hdr.length[1] as u16), ipv6_hdr.nexthdr);
    }
  }
}


pub struct Icmpv6InLocal;

impl Icmpv6InLocal {
  pub const fn new() -> Icmpv6InLocal {
    Icmpv6InLocal {}
  }
}

#[repr(C,packed)]
struct Icmpv6Packet {
  icmp_type: u8,
  code: u8,
  checksum: u16,
  identifier: u16,
  sequence: u16,
}

impl ProcessingNode for Icmpv6InLocal {
  fn process(&self, buff: &[DataFromNetif]) {
    for frame in buff.iter() {
      let slice = frame.get_buffer().slice();
      let ipv6_hdr = unsafe { &*((&slice[14] as *const _) as *const Ipv6Packet) };
      let icmpv6_hdr = unsafe { &*((&slice[14+40] as *const _) as *const Icmpv6Packet) };
      let src_ip_addr = Ipv6Address::from_array(ipv6_hdr.src_ip);
      let dest_ip_addr = Ipv6Address::from_array(ipv6_hdr.dest_ip);

      //println!("ICMPv6 Type={}", icmpv6_hdr.icmp_type);
      match icmpv6_hdr.icmp_type {
        0x80 => {
          //println!("ICMPv6 Echo Request");
          //Echo Request
          //reply
          let length = (ipv6_hdr.length[0] as usize) << 8 | ipv6_hdr.length[1] as usize;
          let netif = Arc::clone(frame.get_netif());
          let respbuff = netif.pre_xmit(14+40+length);
          let respslice = respbuff.slice_mut();

          let dest_mac = MacAddress::new(slice[6..12].try_into().unwrap());
          generate_ether_header(&mut respslice[0..], *netif.get_macaddress(), dest_mac, [0x86, 0xdd]);
          generate_ipv6_header(&mut respslice[14..], ipv6_hdr.length, 58, Ipv6Address::from_array([0x02; 16]), Ipv6Address::from_array(ipv6_hdr.src_ip));

          let mut icmpv6_send_hdr = unsafe { &mut *((&mut respslice[14+40] as *mut _) as *mut Icmpv6Packet) };
          icmpv6_send_hdr.icmp_type = 0x81;
          icmpv6_send_hdr.code = icmpv6_hdr.code;
          icmpv6_send_hdr.checksum = 0;
          icmpv6_send_hdr.identifier = icmpv6_hdr.identifier;
          icmpv6_send_hdr.sequence = icmpv6_hdr.sequence;
          for i in (14+40+8)..(14+40+length) {
            respslice[i] = slice[i];
          }

          let mut csum_icmp: u32 = 0;
          for i in 0..8 {
            csum_icmp = csum_icmp + (0x2 << 8 | 0x02);
          }
          for i in 0..8 {
            csum_icmp = csum_icmp + ((ipv6_hdr.src_ip[i*2] as u32) << 8 | ipv6_hdr.src_ip[i*2+1] as u32);
          }
          csum_icmp = csum_icmp + length as u32;
          csum_icmp = csum_icmp + 58;

          for i in ((14+40)/2)..((14+40+length)/2) {
            csum_icmp = csum_icmp + ((respslice[i*2] as u32) << 8 | (respslice[i*2+1] as u32))
          }
          csum_icmp = (csum_icmp & 0x0000ffff) + (csum_icmp >> 16);
          csum_icmp = (csum_icmp & 0x0000ffff) + (csum_icmp >> 16);
          csum_icmp = !csum_icmp;
          icmpv6_send_hdr.checksum = ((csum_icmp & 0xff) << 8 | (csum_icmp & 0xff00) >> 8) as u16;

          println!("ICMPv6 Echo Request {:08x}", csum_icmp);
            
          netif.xmit(respbuff);
        },
        /*
        0x87 => {
          //neighbor solicitation
          //reply
          let netif = Arc::clone(frame.get_netif());
          let respbuff = netif.pre_xmit(14+40+32);
          let respslice = respbuff.slice_mut();

          let dest_mac = MacAddress::new(slice[6..12].try_into().unwrap());
          generate_ether_header(&mut respslice[0..], *netif.get_macaddress(), dest_mac, [0x86, 0xdd]);
          //todo: source ip
          generate_ipv6_header(&mut respslice[14..], 32, 58, Ipv6Address::from_array([0x02; 16]), Ipv6Address::from_array(ipv6_hdr.src_ip));

          let mut icmpv6_send_hdr = unsafe { &mut *((&mut respslice[14+40] as *mut _) as *mut Icmpv6Packet) };
          icmpv6_send_hdr.icmp_type = 0x81;
          icmpv6_send_hdr.code = icmpv6_hdr.code;
          icmpv6_send_hdr.checksum = 0;
          icmpv6_send_hdr.identifier = icmpv6_hdr.identifier;
          icmpv6_send_hdr.sequence = icmpv6_hdr.sequence;
          for i in (14+40+8)..(14+40+length) {
            respslice[i] = slice[i];
          }

          let mut csum_icmp: u32 = 0;
          for i in 0..8 {
            csum_icmp = csum_icmp + (0x2 << 8 | 0x02);
          }
          for i in 0..8 {
            csum_icmp = csum_icmp + ((ipv6_hdr.src_ip[i*2] as u32) << 8 | ipv6_hdr.src_ip[i*2+1] as u32);
          }
          csum_icmp = csum_icmp + length as u32;
          csum_icmp = csum_icmp + 58;

          for i in ((14+40)/2)..((14+40+length)/2) {
            csum_icmp = csum_icmp + ((respslice[i*2] as u32) << 8 | (respslice[i*2+1] as u32))
          }
          csum_icmp = (csum_icmp & 0x0000ffff) + (csum_icmp >> 16);
          csum_icmp = (csum_icmp & 0x0000ffff) + (csum_icmp >> 16);
          csum_icmp = !csum_icmp;
          icmpv6_send_hdr.checksum = ((csum_icmp & 0xff) << 8 | (csum_icmp & 0xff00) >> 8) as u16;

          println!("ICMPv6 Echo Request {:08x}", csum_icmp);
            
          netif.xmit(respbuff);
        },
        */
        0x88 => {
          //neighbor advertisement
          //register source to fib and adj
          let flags = &slice[14+40+8..14+40+12];
          let src_mac = MacAddress::new([
            slice[14+40+30],
            slice[14+40+31],
            slice[14+40+32],
            slice[14+40+33],
            slice[14+40+34],
            slice[14+40+35],
          ]);
          let src_ip = Ipv6Address::from_array([
            slice[14+40+12], slice[14+40+13], slice[14+40+14], slice[14+40+15], slice[14+40+16], slice[14+40+17], slice[14+40+18], slice[14+40+19],
            slice[14+40+20], slice[14+40+21], slice[14+40+22], slice[14+40+23], slice[14+40+24], slice[14+40+25], slice[14+40+26], slice[14+40+27],
          ]);

          register_ipv6_fib(src_ip, 0xffffffff, 
            src_mac, src_ip, Arc::clone(frame.get_netif()), FIBType::AdjacentResolved
          );
          register_ipv6_adjacent(src_ip, src_mac, Arc::clone(frame.get_netif()), false, None);
        },
        _ => (),
      }
    }
  }
}
