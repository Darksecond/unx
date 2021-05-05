use bootinfo::memory_layout::{HEAP_BASE};
use x86_64::{VirtAddr, structures::paging::{Page, PageTableFlags}};

use self::bump::BumpAllocator;

use super::{Locked, phys::PhysAlloc};

mod bump;

#[global_allocator]
static ALLOCATOR: Locked<BumpAllocator> = Locked::new(BumpAllocator::new());

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("Allocation error {:?}", layout)
}

pub fn init() {
    use x86_64::structures::paging::FrameAllocator;
    unsafe {
        let page = Page::containing_address(VirtAddr::new(HEAP_BASE));
        let frame = PhysAlloc{}.allocate_frame().unwrap();
        
        super::mapper::kernel_map_to(page, frame, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE);
        ALLOCATOR.lock().init(HEAP_BASE, 0x1000);
    }
}