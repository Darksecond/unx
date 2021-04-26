use core::cell::Cell;
use uefi::{prelude::BootServices, table::boot::{AllocateType, MemoryType}};
use uefi::ResultExt;
use x86_64::{PhysAddr, structures::paging::FrameAllocator};
use x86_64::structures::paging::Size4KiB;
use x86_64::structures::paging::PhysFrame;

#[derive(Debug)]
pub struct BootFrameAllocator {
    end: PhysFrame,
    next: Cell<PhysFrame>,
}

impl BootFrameAllocator {
    pub fn new(boot_services: &BootServices, num_frames: usize) -> BootFrameAllocator {
        //TODO Use different MemoryType?
        let frames_addr = boot_services.allocate_pages(AllocateType::AnyPages, MemoryType::RUNTIME_SERVICES_DATA, num_frames).expect_success("Could not allocate boot frames");

        let next: PhysFrame = PhysFrame::containing_address(PhysAddr::new(frames_addr));
        let end: PhysFrame = next + num_frames as _;

        BootFrameAllocator {
            end,
            next: Cell::new(next),
        }
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootFrameAllocator {
    fn allocate_frame(&mut self) -> Option<x86_64::structures::paging::PhysFrame<Size4KiB>> {
        if self.next.get() == self.end {
            return None;
        }

        let frame = self.next.get();
        self.next.set(frame + 1);

        Some(frame)
    }
}