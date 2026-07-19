//! smoltcp `Device` glue over the virtio NIC (P10). The raw-pointer token
//! pattern is the standard one for virtio-drivers: smoltcp hands out an
//! (rx, tx) token pair borrowing the same device and consumes them
//! sequentially on a single hart — never concurrently.

use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::time::Instant;
use virtio_drivers::device::net::RxBuffer;

use super::Nic;

pub struct SmolNic {
    nic: Nic,
}

impl SmolNic {
    pub fn new(nic: Nic) -> Self {
        Self { nic }
    }
}

pub struct VRxToken {
    buf: RxBuffer,
    nic: *mut Nic,
}

pub struct VTxToken {
    nic: *mut Nic,
}

impl Device for SmolNic {
    type RxToken<'a>
        = VRxToken
    where
        Self: 'a;
    type TxToken<'a>
        = VTxToken
    where
        Self: 'a;

    fn receive(&mut self, _ts: Instant) -> Option<(VRxToken, VTxToken)> {
        if !self.nic.can_recv() {
            return None;
        }
        let buf = self.nic.receive().ok()?;
        let nic: *mut Nic = &mut self.nic;
        Some((VRxToken { buf, nic }, VTxToken { nic }))
    }

    fn transmit(&mut self, _ts: Instant) -> Option<VTxToken> {
        if !self.nic.can_send() {
            return None;
        }
        Some(VTxToken { nic: &mut self.nic })
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ethernet;
        caps.max_transmission_unit = 1514;
        caps.max_burst_size = Some(1);
        caps
    }
}

impl RxToken for VRxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        let r = f(self.buf.packet());
        // SAFETY: single hart (D4); the token is consumed exactly once inside
        // the poll that created it, while the owning SmolNic is alive and not
        // otherwise borrowed — smoltcp uses the rx/tx pair sequentially.
        unsafe {
            let _ = (*self.nic).recycle_rx_buffer(self.buf);
        }
        r
    }
}

impl TxToken for VTxToken {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        // SAFETY: as above — single hart, sequential token use, device alive.
        let nic = unsafe { &mut *self.nic };
        let mut tx = nic.new_tx_buffer(len);
        let r = f(tx.packet_mut());
        let _ = nic.send(tx);
        r
    }
}
