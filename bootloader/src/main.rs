#![no_std]
#![no_main]
#![feature(asm)]
#![feature(abi_efiapi)]

mod file;
mod load_kernel;
mod memory;

use bootinfo::{
    boot_info::{BootInfo, FrameBuffer, FrameBufferInfo},
    memory_layout::PHYSMAP_BASE,
};
use load_kernel::{load_kernel, map_area_and_ignore, BOOTLOADER_DATA};
use log::info;
use uefi::{
    prelude::{entry, Boot, BootServices, Handle, Status, SystemTable},
    proto::console::gop::GraphicsOutput,
    table::boot::{AllocateType, MemoryDescriptor, MemoryType},
    ResultExt,
};
use x86_64::{PhysAddr, VirtAddr, structures::paging::{
        mapper::MapperAllSizes, FrameAllocator, OffsetPageTable, Page, PageTable, PageTableFlags,
        PhysFrame, Size2MiB, Size4KiB,
    }};

use crate::memory::{allocate_frames, BootInfoPageAllocator};

fn map_framebuffer<M, A>(
    bootinfo_allocator: &mut BootInfoPageAllocator,
    mapper: &mut M,
    allocator: &mut A,
    boot_services: &BootServices,
) -> FrameBuffer
where
    M: MapperAllSizes,
    A: FrameAllocator<Size4KiB>,
{
    let gop = boot_services
        .locate_protocol::<GraphicsOutput>()
        .expect_success("Could not locate GOP");
    let gop = unsafe { &mut *gop.get() };

    let mode_info = gop.current_mode_info();
    let mut framebuffer = gop.frame_buffer();

    let (base, num_frames) = bootinfo_allocator.allocate(framebuffer.size());
    let frame: PhysFrame =
        PhysFrame::containing_address(PhysAddr::new(framebuffer.as_mut_ptr() as _));

    unsafe {
        map_area_and_ignore(
            mapper,
            base,
            frame,
            num_frames,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
            allocator,
        )
        .expect("Could not map framebuffer");
    }

    FrameBuffer {
        buffer_base: base.start_address().as_u64(),
        buffer_size: framebuffer.size(),
        info: FrameBufferInfo {
            height: mode_info.resolution().1,
            width: mode_info.resolution().0,
            stride: mode_info.stride(),
        },
    }
}

fn map_bootinfo<'a, M, A>(
    bootinfo_allocator: &mut BootInfoPageAllocator,
    mapper: &mut M,
    allocator: &mut A,
    boot_services: &BootServices,
) -> (&'a mut BootInfo, Page)
where
    M: MapperAllSizes,
    A: FrameAllocator<Size4KiB>,
{
    use core::mem;

    let (boot_info_addr, num_frames) = bootinfo_allocator.allocate(mem::size_of::<BootInfo>());

    let frame = allocate_frames(boot_services, num_frames);

    unsafe {
        map_area_and_ignore(
            mapper,
            boot_info_addr,
            frame,
            num_frames,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
            allocator,
        )
        .expect("Could not map boot info");
    }

    let boot_info: &'static mut BootInfo =
        unsafe { &mut *(frame.start_address().as_u64() as *mut BootInfo) };
    (boot_info, boot_info_addr)
}

fn allocate_kernel_page_table(boot_services: &BootServices) -> OffsetPageTable<'static> {
    let phys_offset = VirtAddr::new(0);
    let kernel_page_table_frame = boot_services
        .allocate_pages(AllocateType::AnyPages, BOOTLOADER_DATA, 1)
        .expect_success("Could not allocate kernel page table");

    let addr = phys_offset + kernel_page_table_frame;
    let ptr = addr.as_mut_ptr();
    unsafe {
        *ptr = PageTable::new();
    }

    let table = unsafe { &mut *ptr };

    unsafe { OffsetPageTable::new(table, phys_offset) }
}

fn map_loader<'a, M, A>(
    memory_map: impl Iterator<Item = &'a MemoryDescriptor>,
    allocator: &mut A,
    page_table: &mut M,
) where
    M: MapperAllSizes,
    A: FrameAllocator<Size4KiB>,
{
    //TODO Merge LOADER_CODE and LOADER_DATA into one block
    for entry in memory_map {
        match entry.ty {
            MemoryType::LOADER_CODE => {
                let page: Page = Page::containing_address(VirtAddr::new(entry.phys_start));
                let frame: PhysFrame =
                    PhysFrame::containing_address(PhysAddr::new(entry.phys_start));
                let flags = PageTableFlags::PRESENT;

                unsafe {
                    map_area_and_ignore(
                        page_table,
                        page,
                        frame,
                        entry.page_count,
                        flags,
                        allocator,
                    )
                    .expect("Could not allocate loader code");
                }
            }
            MemoryType::LOADER_DATA => {
                let page: Page = Page::containing_address(VirtAddr::new(entry.phys_start));
                let frame: PhysFrame =
                    PhysFrame::containing_address(PhysAddr::new(entry.phys_start));
                let flags =
                    PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE | PageTableFlags::WRITABLE;

                unsafe {
                    map_area_and_ignore(
                        page_table,
                        page,
                        frame,
                        entry.page_count,
                        flags,
                        allocator,
                    )
                    .expect("Could not allocate loader data");
                }
            }
            _ => {}
        }
    }
}

fn map_stack<M, A>(allocator: &mut A, page_table: &mut M, boot_services: &BootServices) -> u64
where
    M: MapperAllSizes,
    A: FrameAllocator<Size4KiB>,
{
    use bootinfo::memory_layout::{STACK_BASE, STACK_FRAMES, STACK_TOP};

    let frame = allocate_frames(boot_services, STACK_FRAMES);
    let page: Page = Page::containing_address(VirtAddr::new(STACK_BASE));
    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

    unsafe {
        map_area_and_ignore(page_table, page, frame, STACK_FRAMES, flags, allocator)
            .expect("Could not map stack");
    }

    //TODO Double check this.
    VirtAddr::new(STACK_TOP - 1).align_down(8u64).as_u64()
}

fn context_switch(
    page_table: &mut OffsetPageTable,
    entry: u64,
    stack: u64,
    boot_info_addr: *mut BootInfo,
) -> ! {
    let pt = page_table.level_4_table() as *const PageTable as u64;
    unsafe {
        asm!("mov cr3, {}; mov rsp, {}; push 0; jmp {}", 
        in(reg) pt,
        in(reg) stack,
        in(reg) entry,
        in("rdi") boot_info_addr as *const _ as usize);
    }

    unreachable!();
}

#[entry]
fn efi_main(image: Handle, st: SystemTable<Boot>) -> Status {
    uefi_services::init(&st).expect_success("Failed to initialize utils");

    info!("Hello, World!");

    let mut allocator = memory::BootFrameAllocator::new(st.boot_services(), 64);
    let mut kernel_page_table = allocate_kernel_page_table(st.boot_services());
    let mut bootinfo_allocator = BootInfoPageAllocator::new();

    // Load kernel
    let entry = {
        let kernel = file::load_file(image, &st, "kernel.elf");

        let entry = load_kernel(
            kernel.as_slice(),
            st.boot_services(),
            &mut kernel_page_table,
            &mut allocator,
        )
        .expect("Could not load kernel");

        kernel.free(&st);

        entry
    };

    let (_boot_info, boot_info_addr) = {
        let (boot_info, boot_info_addr) = map_bootinfo(
            &mut bootinfo_allocator,
            &mut kernel_page_table,
            &mut allocator,
            st.boot_services(),
        );

        boot_info.frame_buffer = map_framebuffer(
            &mut bootinfo_allocator,
            &mut kernel_page_table,
            &mut allocator,
            st.boot_services(),
        );

        (boot_info, boot_info_addr.start_address().as_mut_ptr())
    };

    let stack = map_stack(&mut allocator, &mut kernel_page_table, st.boot_services());

    {
        use core::mem;
        use core::slice;
        use uefi::table::boot::MemoryDescriptor;

        let mmap_storage = {
            let max_mmap_size =
                st.boot_services().memory_map_size() + 8 * mem::size_of::<MemoryDescriptor>();
            let ptr = st
                .boot_services()
                .allocate_pool(MemoryType::LOADER_DATA, max_mmap_size)?
                .log();
            unsafe { slice::from_raw_parts_mut(ptr, max_mmap_size) }
        };

        let (_system_table, memory_map) = st
            .exit_boot_services(image, mmap_storage)
            .expect_success("Failed to exit boot services");

        map_loader(memory_map.clone(), &mut allocator, &mut kernel_page_table);
        map_physmap(&mut allocator, &mut kernel_page_table, memory_map.clone());
        //TODO memory_map -> boot_info
    }

    context_switch(&mut kernel_page_table, entry, stack, boot_info_addr);
}

fn map_physmap<M, A, I>(allocator: &mut A, mapper: &mut M, memory_map: I)
where
    M: MapperAllSizes,
    A: FrameAllocator<Size4KiB>,
    I: ExactSizeIterator<Item = &'static MemoryDescriptor> + Clone,
{
    let phys_top = memory_map
        .map(|r| PhysAddr::new(r.phys_start) + (r.page_count * 0x1000))
        .max()
        .unwrap().as_u64();

    let offset = VirtAddr::new(PHYSMAP_BASE);
    let start = PhysFrame::containing_address(PhysAddr::new(0));
    let end = PhysFrame::<Size2MiB>::containing_address(PhysAddr::new(phys_top));
    for frame in PhysFrame::range_inclusive(start, end) {
        let page = Page::<Size2MiB>::containing_address(offset + frame.start_address().as_u64());
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;
        unsafe {
            mapper
                .map_to(page, frame, flags, allocator)
                .unwrap()
                .ignore();
        }
    }
}