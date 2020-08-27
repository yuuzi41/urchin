use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::devices::buffer::Buffer;
use crate::net::{DataFromNetif, ProcessingNode};
use crate::net::ethernet::{MacAddress, generate_ether_header};
use crate::net::ipv4::{Ipv4Address};
use crate::net::fib::{FIBType, MAC_ADDR_TABLE, AdjacentInformation, register_macaddress, register_ipv4_fib, register_ipv4_adjacent, IPV4_ADJACENT};

pub struct ArpIn;

impl ArpIn {
  pub const fn new() -> ArpIn {
    ArpIn {}
  }
}

#[repr(C,packed)]
struct ArpPacket {
  htype: u16,
  ptype: u16,
  hlen: u8,
  plen: u8,
  oper: u16,
  sha: [u8; 6],
  spa: [u8; 4],
  tha: [u8; 6],
  tpa: [u8; 4],
}

pub fn generate_arp_packet(buffer: &mut [u8], oper: u16, src_mac: MacAddress, src_ip: Ipv4Address, dest_mac: MacAddress, dest_ip: Ipv4Address) {
  let mut arp_packet = unsafe { &mut *((&mut buffer[0] as *mut _) as *mut ArpPacket) };
  arp_packet.htype = 0x0100;
  arp_packet.ptype = 0x0008;
  arp_packet.hlen = 6;
  arp_packet.plen = 4;
  arp_packet.oper = oper;
  arp_packet.sha = src_mac.get_array();
  arp_packet.spa = src_ip.get_array();
  arp_packet.tha = dest_mac.get_array();
  arp_packet.tpa = dest_ip.get_array();
}

impl ProcessingNode for ArpIn {
  fn process(&self, buff: &[DataFromNetif]) {
    for frame in buff.iter() {
      let slice = frame.get_buffer().slice();
      let arp_packet = unsafe { &*((&slice[14] as *const _) as *const ArpPacket) };

      //println!("DEBUG: oper={} sha={:?} spa={:?} tha={:?} tpa={:?}", arp_packet.oper, arp_packet.sha, arp_packet.spa, arp_packet.tha, arp_packet.tpa);

      {
        //register source to fib and adj
        let src_mac = MacAddress::new(arp_packet.sha);
        let src_ip = Ipv4Address::from_array(arp_packet.spa);

        register_ipv4_fib(src_ip, 0xffffffff, 
          src_mac, src_ip, Arc::clone(frame.get_netif()), FIBType::AdjacentResolved
        );
        register_ipv4_adjacent(src_ip, src_mac, Arc::clone(frame.get_netif()), false, None);
      }


      {
        let dest_ip = Ipv4Address::from_array(arp_packet.tpa);
        let adjtable = IPV4_ADJACENT.lock();
        if let Some(adj) = adjtable.get(&dest_ip) {
          if adj.is_local() {
            match arp_packet.oper {
              0x0100 => {
                //request
                //println!("Received packet is ARP Request");
                let netif = Arc::clone(adj.get_netif());
                let src_mac = MacAddress::new(arp_packet.sha);
                let src_ip = Ipv4Address::from_array(arp_packet.spa);
                let respbuff = netif.pre_xmit(14+28);
                let respslice = respbuff.slice_mut();

                generate_ether_header(&mut respslice[0..], *netif.get_macaddress(), src_mac, [0x08, 0x06]);
                generate_arp_packet(&mut respslice[14..], 0x0200, *netif.get_macaddress(), dest_ip, src_mac, src_ip);
                netif.xmit(respbuff);
              },
              0x0200 => {
                //response, no-op
                //println!("Received packet is ARP Response");
              },
              _ => {
                //something wrong
              },
            }
          }
        }
      }
    }
  }
}
