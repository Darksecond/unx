use log::info;
use uefi::prelude::BootServices;
use uefi::ResultExt;
use xmas_elf::{ElfFile, header, program};
use x86_64::VirtAddr;
use x86_64::structures::paging::Page;
use x86_64::structures::paging::Size4KiB;
use x86_64::structures::paging::PhysFrame;
use x86_64::PhysAddr;
use x86_64::structures::paging::mapper::MapperAllSizes;
use x86_64::structures::paging::FrameAllocator;
use x86_64::structures::paging::PageTableFlags;

pub fn load_kernel<M, F>(buffer: &[u8], boot_services: &BootServices, page_table: &mut M, allocator: &mut F) -> Result<u64, &'static str> where M: MapperAllSizes, F: FrameAllocator<Size4KiB> {
    let elf = ElfFile::new(buffer)?;
    header::sanity_check(&elf)?;

    for pheader in elf.program_iter() {
        use uefi::table::boot::{AllocateType, MemoryType};
        program::sanity_check(pheader, &elf)?;

        match pheader.get_type()? {
            program::Type::Load => {
                assert!(pheader.align() == Page::<Size4KiB>::SIZE);

                let vstart = VirtAddr::new(pheader.virtual_addr());
                let vstart_page: Page = Page::containing_address(vstart);

                let mem_size = x86_64::align_up(pheader.mem_size(), Page::<Size4KiB>::SIZE);
                let num_frames = mem_size / Page::<Size4KiB>::SIZE;

                let frame_addr = boot_services.allocate_pages(AllocateType::AnyPages, MemoryType::RUNTIME_SERVICES_DATA, num_frames as _).expect_success("Could not allocate frames");

                unsafe {
                    let dst_offset = vstart-vstart_page.start_address();
                    info!("{:x?}", dst_offset);
                    let destination = core::slice::from_raw_parts_mut((frame_addr+dst_offset) as usize as *mut u8, pheader.mem_size() as usize);
                    let source = {
                        let start: usize = pheader.offset() as _;
                        let end: usize = (pheader.offset() + pheader.file_size()) as _;
                        info!("{:x} {:x}", start, end);
                        &buffer[start..end]
                    };
                    destination.fill(0);
                    if pheader.file_size() > 0 {
                        destination.copy_from_slice(source);
                    }
                }

                //map vstart to frame_addr for num_frames
                for frame in 0..num_frames {
                    let virt = vstart_page + frame;
                    let phys: PhysFrame = PhysFrame::containing_address(PhysAddr::new(frame_addr)) + frame;

                    info!("{:?} -> {:?}", virt, phys);
                    
                    let mut flags = PageTableFlags::PRESENT;
                    if pheader.flags().is_write() { 
                        flags |= PageTableFlags::WRITABLE;
                    }
                    if !pheader.flags().is_execute() {
                        flags |= PageTableFlags::NO_EXECUTE;
                    }

                    unsafe {
                        page_table.map_to(virt, phys, flags, allocator).expect("Could not map frame").ignore();
                    }
                }


                info!("{:x?} {:x?}", pheader, vstart_page+num_frames);
            },
            _ => {},
        }
    }

    Ok(elf.header.pt2.entry_point())
}