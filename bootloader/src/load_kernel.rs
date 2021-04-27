use uefi::prelude::BootServices;
use uefi::ResultExt;
use x86_64::structures::paging::mapper::MapperAllSizes;
use x86_64::structures::paging::FrameAllocator;
use x86_64::structures::paging::Page;
use x86_64::structures::paging::PageTableFlags;
use x86_64::structures::paging::PhysFrame;
use x86_64::structures::paging::Size4KiB;
use x86_64::PhysAddr;
use x86_64::{structures::paging::mapper::MapToError, VirtAddr};
use xmas_elf::{header, program, ElfFile};

//TODO This needs a better name
pub const BOOTLOADER_DATA: uefi::table::boot::MemoryType = uefi::table::boot::MemoryType::custom(0x80000000);

pub unsafe fn map_area_and_ignore<M, F>(
    mapper: &mut M,
    page: Page,
    frame: PhysFrame,
    count: u64,
    flags: PageTableFlags,
    allocator: &mut F,
) -> Result<(), MapToError<Size4KiB>>
where
    M: MapperAllSizes,
    F: FrameAllocator<Size4KiB>,
{
    for i in 0..count {
        mapper
            .map_to(page + i, frame + i, flags, allocator)?
            .ignore();
    }

    Ok(())
}

pub fn load_kernel<M, F>(
    buffer: &[u8],
    boot_services: &BootServices,
    page_table: &mut M,
    allocator: &mut F,
) -> Result<u64, &'static str>
where
    M: MapperAllSizes,
    F: FrameAllocator<Size4KiB>,
{
    let elf = ElfFile::new(buffer)?;
    header::sanity_check(&elf)?;

    for pheader in elf.program_iter() {
        use uefi::table::boot::{AllocateType};
        program::sanity_check(pheader, &elf)?;

        match pheader.get_type()? {
            program::Type::Load => {
                assert!(pheader.align() == Page::<Size4KiB>::SIZE);

                log::info!("{:X?}", pheader);

                let vstart = VirtAddr::new(pheader.virtual_addr());
                let vstart_page: Page = Page::containing_address(vstart);

                let mem_size = x86_64::align_up(pheader.mem_size(), Page::<Size4KiB>::SIZE);
                let num_frames = mem_size / Page::<Size4KiB>::SIZE;

                let frame_addr = boot_services
                    .allocate_pages(
                        AllocateType::AnyPages,
                        BOOTLOADER_DATA,
                        num_frames as _,
                    )
                    .expect_success("Could not allocate frames");

                unsafe {
                    let dst_offset = vstart - vstart_page.start_address();
                    let destination = core::slice::from_raw_parts_mut(
                        (frame_addr + dst_offset) as usize as *mut u8,
                        pheader.mem_size() as usize,
                    );
                    let source = {
                        let start: usize = pheader.offset() as _;
                        let end: usize = (pheader.offset() + pheader.file_size()) as _;
                        &buffer[start..end]
                    };

                    destination.fill(0);

                    if pheader.file_size() > 0 {
                        destination.copy_from_slice(source);
                    }
                }

                // Map segment frames to pages
                {
                    let phys: PhysFrame = PhysFrame::containing_address(PhysAddr::new(frame_addr));

                    let mut flags = PageTableFlags::PRESENT;
                    if pheader.flags().is_write() {
                        flags |= PageTableFlags::WRITABLE;
                    }
                    if !pheader.flags().is_execute() {
                        flags |= PageTableFlags::NO_EXECUTE;
                    }

                    unsafe {
                        map_area_and_ignore(
                            page_table,
                            vstart_page,
                            phys,
                            num_frames,
                            flags,
                            allocator,
                        )
                        .expect("Could not allocate kernel segment");
                    }
                }
            }
            _ => {}
        }
    }

    Ok(elf.header.pt2.entry_point())
}
