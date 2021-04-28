#![no_std]
#![no_main]
#![feature(asm)]
#![feature(abi_efiapi)]

mod allocator;
mod load_kernel;

use core::panic::PanicInfo;

use bootinfo::boot_info::{BootInfo, FrameBuffer, FrameBufferInfo};
use load_kernel::{load_kernel, map_area_and_ignore, BOOTLOADER_DATA};
use log::info;
use uefi::{
    prelude::{entry, Boot, BootServices, Handle, Status, SystemTable},
    proto::console::gop::GraphicsOutput,
    table::boot::{AllocateType, MemoryDescriptor, MemoryType},
    ResultExt,
};
use x86_64::{
    align_up,
    structures::paging::{
        mapper::MapperAllSizes, FrameAllocator, OffsetPageTable, Page, PageTable, PageTableFlags,
        PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};

fn map_framebuffer<M, A>(
    base: Page,
    mapper: &mut M,
    allocator: &mut A,
    boot_services: &BootServices,
) -> (Page, FrameBuffer)
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

    let num_frames =
        align_up(framebuffer.size() as _, Page::<Size4KiB>::SIZE) / Page::<Size4KiB>::SIZE;
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

    info!("{:X?}", mode_info);

    (
        base + num_frames,
        FrameBuffer {
            buffer_base: base.start_address().as_u64(),
            buffer_size: framebuffer.size(),
            info: FrameBufferInfo {
                height: mode_info.resolution().1,
                width: mode_info.resolution().0,
                stride: mode_info.stride(),
            },
        },
    )
}

fn map_bootinfo<'a, M, A>(
    base: Page,
    mapper: &mut M,
    allocator: &mut A,
    boot_services: &BootServices,
) -> (Page, &'a mut BootInfo, Page)
where
    M: MapperAllSizes,
    A: FrameAllocator<Size4KiB>,
{
    let num_frames = align_up(core::mem::size_of::<BootInfo>() as _, Page::<Size4KiB>::SIZE) / Page::<Size4KiB>::SIZE;

    let frame = boot_services.allocate_pages(AllocateType::AnyPages, BOOTLOADER_DATA, num_frames as _).expect_success("Could not allocate memory for boot info");
    let frame: PhysFrame = PhysFrame::containing_address(PhysAddr::new(frame));

    unsafe {
        map_area_and_ignore(mapper, base, frame, num_frames, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE, allocator).expect("Could not map boot info");
    }

    let boot_info: &'static mut BootInfo = unsafe { &mut *(frame.start_address().as_u64() as *mut BootInfo) };
    (base + num_frames, boot_info, base)
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

    let stack_pool = boot_services
        .allocate_pages(AllocateType::AnyPages, BOOTLOADER_DATA, STACK_FRAMES as _)
        .expect_success("Could not allocate stack");

    let frame: PhysFrame = PhysFrame::containing_address(PhysAddr::new(stack_pool));
    let page: Page = Page::containing_address(VirtAddr::new(STACK_BASE));
    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

    unsafe {
        map_area_and_ignore(page_table, page, frame, STACK_FRAMES, flags, allocator)
            .expect("Could not map stack");
    }

    //TODO Double check this.
    VirtAddr::new(STACK_TOP - 1).align_down(8u64).as_u64()
}

fn context_switch(page_table: &mut OffsetPageTable, entry: u64, stack: u64, boot_info_addr: *mut BootInfo) -> ! {
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

// #[panic_handler]
// fn panic(info: &PanicInfo) -> ! {
//     log::error!("{}", info);
//     loop {
//         unsafe { asm!("cli; hlt") };
//     }
// }

#[entry]
fn efi_main(image: Handle, st: SystemTable<Boot>) -> Status {
    uefi_services::init(&st).expect_success("Failed to initialize utils");

    info!("Hello, World!");

    let mut allocator = allocator::BootFrameAllocator::new(st.boot_services(), 64);
    let mut kernel_page_table = allocate_kernel_page_table(st.boot_services());

    // Load kernel
    let entry = {
        let kernel = load_file(image, &st, "kernel.elf");

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

    let boot_info_addr: *mut BootInfo = {
        use bootinfo::memory_layout::BOOTINFO_BASE;

        let base = Page::containing_address(VirtAddr::new(BOOTINFO_BASE));
        let (base, frame_buffer) = map_framebuffer(
            base,
            &mut kernel_page_table,
            &mut allocator,
            st.boot_services(),
        );
        let (base, boot_info, boot_info_addr) = map_bootinfo(base, &mut kernel_page_table, &mut allocator, st.boot_services());

        boot_info.frame_buffer = frame_buffer;

        info!("{:X?}", frame_buffer);

        boot_info_addr.start_address().as_mut_ptr()
    };

    let stack = { map_stack(&mut allocator, &mut kernel_page_table, st.boot_services()) };

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
        map_loader(memory_map, &mut allocator, &mut kernel_page_table);
    }

    context_switch(&mut kernel_page_table, entry, stack, boot_info_addr);
}

struct LoadedFileBuffer {
    buffer_addr: *mut u8,
    buffer_len: usize,
}

impl LoadedFileBuffer {
    pub fn as_slice(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts_mut(self.buffer_addr as *mut u8, self.buffer_len) }
    }

    pub fn free(self, st: &SystemTable<Boot>) {
        st.boot_services()
            .free_pool(self.buffer_addr)
            .expect_success("Could not free buffer");
    }
}

fn load_file(image: Handle, st: &SystemTable<Boot>, path: &str) -> LoadedFileBuffer {
    use uefi::proto::{
        loaded_image::LoadedImage,
        media::{
            file::{File, FileAttribute, FileInfo, FileMode, FileType},
            fs::SimpleFileSystem,
        },
    };

    let loaded_image = unsafe {
        &mut *st
            .boot_services()
            .handle_protocol::<LoadedImage>(image)
            .expect_success("Failed to open LoadedImage protocol")
            .get()
    };
    let fs = unsafe {
        &mut *st
            .boot_services()
            .handle_protocol::<SimpleFileSystem>(loaded_image.device())
            .expect_success("Failed to open SimpleFileSystem protocol")
            .get()
    };
    let root = &mut fs.open_volume().expect_success("Could not open volume");

    let mut file = root
        .open(path, FileMode::Read, FileAttribute::READ_ONLY)
        .expect_success("Could not open file");

    let mut info_buffer = [0u8; 128];
    let info = file
        .get_info::<FileInfo>(&mut info_buffer)
        .expect_success("Could not get file info");

    // Log filename and size for debugging
    info!("Loading {} ({} bytes)", info.file_name(), info.file_size());

    let buffer_addr = st
        .boot_services()
        .allocate_pool(BOOTLOADER_DATA, info.file_size() as usize)
        .expect_success("Could not allocate memory for file");

    let buffer: &mut [u8] = unsafe {
        core::slice::from_raw_parts_mut(buffer_addr as *mut u8, info.file_size() as usize)
    };

    match file.into_type().expect_success("Could not get file type") {
        FileType::Regular(mut regular_file) => {
            regular_file
                .read(buffer)
                .expect_success("Could not read file");
        }
        FileType::Dir(_) => panic!("file path is a directory"),
    }

    LoadedFileBuffer {
        buffer_addr,
        buffer_len: info.file_size() as usize,
    }
}
