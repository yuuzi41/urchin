use alloc::sync::Arc;
use alloc::vec::Vec;

use crc::crc32;

use super::VirtioDevice;
use super::Virtqueue;
use crate::interrupt;
use crate::spinlock::Spinlock;
use crate::devices::netif;
use crate::devices::netif::Netif;
use crate::devices::buffer::Buffer;
use crate::net;
use crate::PROC_NODES;
use crate::NET_IFACES;

const ALIGN: usize = 4096;

pub struct VirtioNet<'a, T: VirtioDevice> {
  id: usize,

  virtio: Spinlock<T>,
  queues: [Spinlock<Virtqueue<'a>>; 3],
  macaddr: net::ethernet::MacAddress,

  flag_csum: bool,
  flag_guest_csum: bool,
  flag_mac: bool,
  flag_guest_tso4: bool,
  flag_guest_ecn: bool,
  flag_guest_ufo: bool,
  flag_host_tso4: bool,
  flag_host_ecn: bool,
  flag_mergeable_rxbufs: bool,
  flag_status: bool,
}

impl <'a, T: VirtioDevice> VirtioNet<'a, T> {
  pub fn new(id: usize, mut dev: T) -> Option<VirtioNet<'a, T>> {
    if dev.get_type() == 0x01 { //it means virtio-net
      //reset
      dev.set_status(0);
      //acknoledge and driver
      dev.set_status(dev.get_status() | 0x01); //acknowledge
      dev.set_status(dev.get_status() | 0x02); //driver
      //negotiate features
      let dev_features = dev.get_available_features();
      let drv_features_common = 0x30000000;
      /*
       1 << VIRTIO_RING_F_INDIRECT_DESC | 1 << VIRTIO_RING_F_EVENT_IDX; 
      */
      let drv_features_net = 0x1AEA3;
      /*
      | (1 << VIRTIO_NET_F_CSUM)       \  0
      | (1 << VIRTIO_NET_F_GUEST_CSUM) \  1
      | (1 << VIRTIO_NET_F_MAC)        \  5
      | (1 << VIRTIO_NET_F_GUEST_TSO4) \  7
      | (1 << VIRTIO_NET_F_GUEST_ECN)     9
      | (1 << VIRTIO_NET_F_GUEST_UFO)     10
      | (1 << VIRTIO_NET_F_HOST_TSO4)  \  11
      | (1 << VIRTIO_NET_F_HOST_ECN)   \  13
      | (1 << VIRTIO_NET_F_MRG_RXBUF)  \  15
      | (1 << VIRTIO_NET_F_STATUS)     \  16
      */
      let drv_features = drv_features_common | drv_features_net;
      let subset_features = dev_features & drv_features;

      dev.set_indirect_buf_cap((subset_features & 0x10000000) != 0 );
      dev.set_event_idx_cap((subset_features & 0x20000000) != 0);        

      let flag_csum = (subset_features & 0x000001) != 0;
      let flag_guest_csum = (subset_features & 0x000002) != 0;
      let flag_mac = (subset_features & 0x000020) != 0;
      let flag_guest_tso4 = (subset_features & 0x000080) != 0;
      let flag_guest_ecn = (subset_features & 0x000200) != 0;
      let flag_guest_ufo = (subset_features & 0x000400) != 0;
      let flag_host_tso4 = (subset_features & 0x000800) != 0;
      let flag_host_ecn = (subset_features & 0x002000) != 0;
      let flag_mergeable_rxbufs = (subset_features & 0x008000) != 0;
      let flag_status = (subset_features & 0x010000) != 0;

      dev.set_enabled_features(subset_features);
      if dev.is_legacy() == false {
        dev.set_status(dev.get_status() | 0x08);
        assert_eq!(dev.get_status() & 0x08, 0x08);
      }

      let macaddr = if !flag_mac {
        // generate macaddr
        use crate::io;
        let tscnum = unsafe { io::rdtsc_with_cpuid() };
        net::ethernet::MacAddress::new([
          0x00,
          0x16,
          0x3e,
          ((tscnum >> 0 & 0xff) ^ (tscnum >> 40 & 0xff) ^ (tscnum >> 16 & 0xff)) as u8,
          ((tscnum >> 56 & 0xff) ^ (tscnum >> 32 & 0xff) ^ (tscnum >> 8 & 0xff)) as u8,
          ((tscnum >> 48 & 0xff) ^ (tscnum >> 24 & 0xff) ^ (tscnum >> 0 & 0xff)) as u8,
        ])
      } else {
        //get macaddr
        net::ethernet::MacAddress::new([
          dev.read_config(0),
          dev.read_config(1),
          dev.read_config(2),
          dev.read_config(3),
          dev.read_config(4),
          dev.read_config(5),
        ])
      };

      let mut setup_queue = |idx, need_interrupt| {
        dev.select_queue(idx);
        let size = dev.get_queue_size() as usize;
        let queue = Virtqueue::new(size as u16, dev.get_indirect_buf_cap(), need_interrupt);
        dev.setup_queue(&queue);
        dev.activate_queue(idx);
        Spinlock::new(queue)
      };

      let rxqueue = setup_queue(0, true);
      let txqueue = setup_queue(1, false);
      let cxqueue = setup_queue(2, true);

      //todo:retrieve device info

      ///////////
      {
        let mut rxq = rxqueue.lock();
        let size = rxq.get_avail_free_size() / 3;  
        let is_large = flag_mergeable_rxbufs && (flag_guest_tso4 || flag_guest_ufo);
        let buffer_size = if is_large {
          17 * ALIGN
        } else {
          1 * ALIGN
        };
  
        for _i in 0..size {
          match Buffer::new(buffer_size, ALIGN) {
            Ok(rawbuffer) => {
              let buffer = Arc::new(rawbuffer);
              match rxq.push(buffer, false) {
                Ok(_) => (),
                Err(_) => break,
              }
            },
            Err(_e) => (),
          }
        }
        dev.kick_queue(0);
      }
      ///////////

      //enable
      dev.set_status(dev.get_status() | 0x04);

      let inst = VirtioNet {
        id: id, 
        virtio: Spinlock::new(dev), queues: [rxqueue, txqueue, cxqueue], macaddr: macaddr, 
        flag_csum: flag_csum,
        flag_guest_csum: flag_guest_csum,
        flag_mac: flag_mac,
        flag_guest_tso4: flag_guest_tso4,
        flag_guest_ecn: flag_guest_ecn,
        flag_guest_ufo: flag_guest_ufo,
        flag_host_tso4: flag_host_tso4,
        flag_host_ecn: flag_host_ecn, 
        flag_mergeable_rxbufs: flag_mergeable_rxbufs,
        flag_status: flag_status,
      };

      //fill rx ring
      //inst.fill_rx_queue();
      
      Some(inst)
    } else {
      None
    }
  }

  fn fill_rx_queue(&self) {
    let rxqidx = 0;
    let mut rxq = self.queues[rxqidx].lock();
    let size = rxq.get_avail_free_size() - 1;
    //println!("free = {}", rxq.get_avail_free_size());

    if size > 32 {
      //println!("Queue refill size={}", size);

      let is_large = self.flag_mergeable_rxbufs && (self.flag_guest_tso4 || self.flag_guest_ufo);
      let buffer_size = if is_large {
        17 * ALIGN
      } else {
        1 * ALIGN
      };

      for _i in 0..size {
        match Buffer::new(buffer_size, ALIGN) {
          Ok(rawbuffer) => {
            let buffer = Arc::new(rawbuffer);
            match rxq.push(buffer, false) {
              Ok(_) => (),
              Err(_) => break,
            }
          },
          Err(_e) => (),
        }
      }

      {
        let mut virtio = self.virtio.lock();
        virtio.kick_queue(rxqidx as u32);
      }
    }
  }

}

impl <'a, T: VirtioDevice> interrupt::Interruptable for VirtioNet<'a, T> {
  fn get_irq(&self) -> u8 {
    let virtio = self.virtio.lock();
    virtio.get_irq()
  }

  fn interrupt_handler(&self) {
    let irqed = {
      let mut virtio = self.virtio.lock();
      virtio.read_and_ack_isr() > 0
    };

    if irqed { 
      //fill rx ring
      self.fill_rx_queue();

      // retrieve data
      let rxdata = {
        let mut rxq = self.queues[0].lock();
        rxq.pop()
      };

      //--------------
      if let Some(self_arc) = unsafe { NET_IFACES.get(self.get_id()) } {
        let mut data = Vec::with_capacity(rxdata.len());
        for pkt in &rxdata {
          pkt.slide_position(12);
          data.push(net::DataFromNetif::new(Arc::clone(self_arc), Arc::clone(&pkt)));
        }

        if let Some(node_ref) = unsafe { PROC_NODES.get("ethernet-in") } {
          node_ref.process(data.as_slice());
        }
      }
      //--------------
    }
  }
}

// i guess i shouldn't use an interface like this.

impl <'a, T: VirtioDevice> netif::Netif for VirtioNet<'a, T> {
  fn pre_xmit(&self, size: usize) -> Arc<Buffer> {
    let actual_size = size + 12; // add size for virtio-net header

    if let Ok(rawbuffer) = Buffer::new(actual_size, ALIGN) {
      let buffer = Arc::new(rawbuffer);
      let slice = buffer.slice_mut();

      slice[0] = 0x00; //flags 
      slice[1] = 0x00; //gso
      slice[2] = 0x00; //hdr_len
      slice[3] = 0x00; //
      slice[4] = 0x00; //gso_size
      slice[5] = 0x00; //
      slice[6] = 0x00; //csum_start
      slice[7] = 0x00; //
      slice[8] = 0x00; //csum_offset
      slice[9] = 0x00; //
      slice[10] = 0x00; //num_buffers
      slice[11] = 0x00; //
      buffer.slide_position(12);

      buffer
    } else {
      panic!("something wrong");
    }
  }

  fn xmit(&self, buffer: Arc<Buffer>) -> Result<(), netif::Error> {
    //MUST disable interrupt
    let txqidx = 1;   
    let mut txq = self.queues[txqidx].lock();

    match txq.push(buffer, true) {
      Ok(_) => {
        let mut virtio = self.virtio.lock();
        virtio.kick_queue(txqidx as u32);
        Ok(())
      }
      Err(_) => Err(netif::Error::TransmitError())
    }
  }

  fn recv(&self) {
    
  }

  fn get_id(&self) -> usize {
    self.id
  }
  
  fn get_macaddress(&self) -> &net::ethernet::MacAddress {
    &self.macaddr
  }

  fn get_drivername(&self) -> &'static str {
    "virtio-net"
  }
}

unsafe impl <'a, T: VirtioDevice> Send for VirtioNet<'a, T> {}
unsafe impl <'a, T: VirtioDevice> Sync for VirtioNet<'a, T> {}
