use core::cell::Cell;
use bootinfo::memory_layout::BOOTINFO_BASE;
use uefi::{prelude::BootServices, table::boot::{AllocateType, MemoryType}};
use uefi::ResultExt;
use x86_64::{PhysAddr, VirtAddr, align_up, structures::paging::{FrameAllocator, Page}};
use x86_64::structures::paging::Size4KiB;
use x86_64::structures::paging::PhysFrame;

use crate::load_kernel::BOOTLOADER_DATA;

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

#[derive(Debug)]
pub struct BootInfoPageAllocator {
    next: Cell<Page>,
}

impl BootInfoPageAllocator {
    pub fn new() -> BootInfoPageAllocator {
        BootInfoPageAllocator {
            next: Cell::new(Page::containing_address(VirtAddr::new(BOOTINFO_BASE))),
        }
    }

    //TODO Check next again BOOTINFO_TOP
    //TODO Option<Page>
    pub fn allocate(&mut self, bytes: usize) -> (Page, u64) {
        let num_frames = align_up(bytes as _, Page::<Size4KiB>::SIZE) / Page::<Size4KiB>::SIZE;

        let page = self.next.get();
        self.next.set(page + num_frames);

        (page, num_frames)
    }
}

pub fn allocate_frames(boot_services: &BootServices, num_frames: u64) -> PhysFrame {
    let frame = boot_services
        .allocate_pages(AllocateType::AnyPages, BOOTLOADER_DATA, num_frames as _)
        .expect_success("Could not allocate memory for boot info");

    PhysFrame::containing_address(PhysAddr::new(frame))
}