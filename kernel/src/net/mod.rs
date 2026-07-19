//! P9 — virtio-net bring-up on QEMU virt (virtio-mmio, polled; PRD v2 D11/D12).
//! All `unsafe` network code lives here (HAL glue + MMIO probe), per CLAUDE.md
//! rule 3. No interrupts: the device is polled from the cooperative loop.

use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};

use virtio_drivers::device::net::VirtIONet;
use virtio_drivers::transport::mmio::{MmioTransport, VirtIOHeader};
use virtio_drivers::transport::{DeviceType, Transport};
use virtio_drivers::{BufferDirection, Hal, PhysAddr};

use crate::println;

/// QEMU virt: 8 virtio-mmio slots, 4 KiB apart.
const VIRTIO_MMIO_BASE: usize = 0x1000_1000;
const VIRTIO_MMIO_STRIDE: usize = 0x1000;
const VIRTIO_MMIO_SLOTS: usize = 8;

// ── DMA arena ────────────────────────────────────────────────────────────────

const PAGE_SIZE: usize = 4096;
const ARENA_PAGES: usize = 64; // 256 KiB — virtqueues + buffer headroom

#[repr(C, align(4096))]
struct Arena([u8; ARENA_PAGES * PAGE_SIZE]);

static mut DMA_ARENA: Arena = Arena([0; ARENA_PAGES * PAGE_SIZE]);
static DMA_NEXT: AtomicUsize = AtomicUsize::new(0);

pub struct RvHal;

// SAFETY (trait contract): dma_alloc returns zeroed, page-aligned, physically
// contiguous memory that stays valid until dealloc; with no MMU, virtual and
// physical addresses coincide, so share/unshare are identity and no bounce
// buffers exist. Single hart (D4) + atomic bump ⇒ no double-allocation.
unsafe impl Hal for RvHal {
    fn dma_alloc(pages: usize, _direction: BufferDirection) -> (PhysAddr, NonNull<u8>) {
        let start = DMA_NEXT.fetch_add(pages, Ordering::Relaxed);
        assert!(start + pages <= ARENA_PAGES, "net DMA arena exhausted");
        // SAFETY: the page range is exclusively ours (bump above), inside the
        // static arena, page-aligned by construction, and zero-initialized —
        // and never reused, because dealloc leaks by design (NIC lives forever).
        let ptr = unsafe { (&raw mut DMA_ARENA.0).cast::<u8>().add(start * PAGE_SIZE) };
        (ptr as usize as PhysAddr, NonNull::new(ptr).unwrap())
    }

    unsafe fn dma_dealloc(_paddr: PhysAddr, _vaddr: NonNull<u8>, _pages: usize) -> i32 {
        0 // leak by design: the NIC has kernel lifetime (P9 note in PLAN)
    }

    unsafe fn mmio_phys_to_virt(paddr: PhysAddr, _size: usize) -> NonNull<u8> {
        // Identity (no MMU). Only the PCI-BAR path uses this; virtio-mmio won't.
        NonNull::new(paddr as *mut u8).unwrap()
    }

    unsafe fn share(buffer: NonNull<[u8]>, _direction: BufferDirection) -> PhysAddr {
        buffer.cast::<u8>().as_ptr() as usize as PhysAddr // identity, no bounce
    }

    unsafe fn unshare(_paddr: PhysAddr, _buffer: NonNull<[u8]>, _direction: BufferDirection) {}
}

// ── Probe ────────────────────────────────────────────────────────────────────

/// RX/TX queue depth 16; 2 KiB packet buffers (> MTU 1500 + virtio-net header).
pub type Nic = VirtIONet<RvHal, MmioTransport<'static>, 16>;

fn probe() -> Option<Nic> {
    for slot in 0..VIRTIO_MMIO_SLOTS {
        let base = VIRTIO_MMIO_BASE + slot * VIRTIO_MMIO_STRIDE;
        let header = NonNull::new(base as *mut VirtIOHeader).unwrap();
        // SAFETY: `base` is a QEMU-virt virtio-mmio slot — valid MMIO for
        // 'static; the constructor validates magic/version and rejects empty
        // slots, so probing every slot is safe.
        let Ok(transport) = (unsafe { MmioTransport::new(header, VIRTIO_MMIO_STRIDE) }) else {
            continue;
        };
        if transport.device_type() != DeviceType::Network {
            continue;
        }
        if let Ok(nic) = Nic::new(transport, 2048) {
            return Some(nic);
        }
    }
    None
}

/// Bring the NIC up and announce it. `None` (no device) is not fatal in P9 —
/// the v1 flow proceeds; P10+ features will require it.
pub fn init() -> Option<Nic> {
    let nic = probe()?;
    let mac = nic.mac_address();
    println!(
        "net: virtio-net up mac={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    );
    Some(nic)
}
