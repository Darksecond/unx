use bootinfo::{boot_info::{MemoryMap, MemoryType}, memory_layout::PHYSMAP_BASE};
use spinning_top::{Spinlock, const_spinlock};
use x86_64::{PhysAddr, VirtAddr, structures::paging::{FrameAllocator, FrameDeallocator, PhysFrame, Size4KiB}};

struct StackElement {
    next: Option<PhysAddr>,
}

fn phys_to_virt(phys: PhysAddr) -> VirtAddr {
    VirtAddr::new(phys.as_u64() + PHYSMAP_BASE)
}

struct StackFrameAllocator {
    next: Option<PhysAddr>,
    count: usize,
}

impl StackFrameAllocator {
    pub const fn new() -> Self {
        StackFrameAllocator {
            next: None,
            count: 0,
        }
    }
}

unsafe impl FrameAllocator<Size4KiB> for StackFrameAllocator {
    fn allocate_frame(&mut self) -> Option<x86_64::structures::paging::PhysFrame<Size4KiB>> {
        if let Some(next) = self.next {
            
            let element: *const StackElement = phys_to_virt(next).as_ptr();
            let element = unsafe {&*element };

            self.next = element.next;
            self.count -= 1;

            Some(PhysFrame::containing_address(next))
        } else {
            None
        }
    }
}

impl FrameDeallocator<Size4KiB> for StackFrameAllocator {
    unsafe fn deallocate_frame(&mut self, frame: x86_64::structures::paging::PhysFrame<Size4KiB>) {
        let element: *mut StackElement = phys_to_virt(frame.start_address()).as_mut_ptr();
        let element = &mut *element;

        element.next = self.next;

        self.next = Some(frame.start_address());
        self.count += 1;
    }
}

static FRAME_ALLOCATOR: Spinlock<StackFrameAllocator> = const_spinlock(StackFrameAllocator::new());

pub fn init(map: &MemoryMap) {
    let mut guard = FRAME_ALLOCATOR.lock();
    for entry in map.entries() {
        if entry.memory_type == MemoryType::Conventional {
            let start = PhysFrame::containing_address(PhysAddr::new(entry.start));
            let end = PhysFrame::containing_address(PhysAddr::new(entry.start + entry.size as u64));
            for frame in PhysFrame::range(start, end) {
                unsafe {
                    guard.deallocate_frame(frame);
                }
            }
        }
    }
}