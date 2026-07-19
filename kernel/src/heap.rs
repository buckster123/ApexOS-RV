//! Global allocator: embedded-alloc (LLFF) over a static region (PRD D5).
//!
//! OOM policy (P4.4): allocation failure takes the default alloc-error path,
//! which panics — our reporting panic handler prints it and exits QEMU with a
//! failure code, so OOM is always visible, never a silent hang.

use core::mem::MaybeUninit;
use embedded_alloc::LlffHeap;

/// 1 MiB normally; 1 KiB under the `tiny-heap` gate feature (P4 negative test).
pub const SIZE: usize = if cfg!(feature = "tiny-heap") { 1024 } else { 0x10_0000 };

#[global_allocator]
static HEAP: LlffHeap = LlffHeap::empty();

static mut HEAP_MEM: [MaybeUninit<u8>; SIZE] = [MaybeUninit::uninit(); SIZE];

/// Call once at boot, before the first allocation.
pub fn init() {
    // SAFETY: HEAP_MEM is reserved solely for the allocator and never touched
    // again outside it; single hart (D4) and one call site at boot before any
    // allocation ⇒ no aliasing, no concurrent or repeated init.
    unsafe { HEAP.init((&raw mut HEAP_MEM) as *mut u8 as usize, SIZE) }
}
