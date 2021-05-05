use bootinfo::memory_layout::PHYSMAP_BASE;
use spinning_top::{Spinlock, const_spinlock};
use x86_64::{VirtAddr, registers::control::Cr3, structures::paging::{OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame}};

use crate::memory::phys::PhysAlloc;

static VIRT_PHYSMAP_OFFSET: VirtAddr = VirtAddr::new_truncate(PHYSMAP_BASE);
static KERNEL_PAGE_TABLE: Spinlock<Option<OffsetPageTable>> = const_spinlock(None);

unsafe fn active_l4() -> &'static mut PageTable {
    let (l4, _) = Cr3::read();

    let phys = l4.start_address();
    let virt = VIRT_PHYSMAP_OFFSET + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr
}

pub unsafe fn kernel_map_to(page: Page, frame: PhysFrame, flags: PageTableFlags) {
    if let Some(mapper) = KERNEL_PAGE_TABLE.lock().as_mut() {
        use x86_64::structures::paging::mapper::Mapper;
        mapper.map_to(page, frame, flags, &mut PhysAlloc).unwrap().flush();
    }
}

pub fn init() {
    unsafe {
        let l4 = active_l4();
        KERNEL_PAGE_TABLE
            .lock()
            .insert(OffsetPageTable::new(l4, VIRT_PHYSMAP_OFFSET));
    }
}