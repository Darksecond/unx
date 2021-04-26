#![no_std]
#![no_main]
#![feature(asm)]
#![feature(abi_efiapi)]

mod allocator;
mod load_kernel;

use load_kernel::load_kernel;
use log::info;
use uefi::{ResultExt, prelude::{entry, Boot, BootServices, Handle, Status, SystemTable}, table::boot::{AllocateType, MemoryDescriptor, MemoryType}};
use x86_64::{PhysAddr, VirtAddr, structures::paging::{FrameAllocator, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame, Size4KiB, mapper::MapperAllSizes}};

fn allocate_kernel_page_table(boot_services: &BootServices) -> OffsetPageTable<'static> {
    let phys_offset = VirtAddr::new(0);
    let kernel_page_table_frame = boot_services
        .allocate_pages(AllocateType::AnyPages, MemoryType::RUNTIME_SERVICES_DATA, 1)
        .expect_success("Could not allocate kernel page table");

    let addr = phys_offset + kernel_page_table_frame;
    let ptr = addr.as_mut_ptr();
    unsafe { *ptr = PageTable::new(); }

    let table = unsafe { &mut *ptr };

    unsafe { OffsetPageTable::new(table, phys_offset) }

}

fn map_loader<'a, M, A>(memory_map: impl Iterator<Item = &'a MemoryDescriptor>, allocator: &mut A, page_table: &mut M) where M: MapperAllSizes, A: FrameAllocator<Size4KiB> {
    for entry in memory_map {
        match entry.ty {
            MemoryType::LOADER_CODE => {
                
                let mut page: Page = Page::containing_address(VirtAddr::new(entry.phys_start));
                let mut frame: PhysFrame = PhysFrame::containing_address(PhysAddr::new(entry.phys_start));

                for i in 0..entry.page_count {
                    page += i;
                    frame += i;
                    write_serial("c");
                    unsafe {
                        page_table.map_to(page, frame, PageTableFlags::PRESENT, allocator).unwrap().ignore();
                    }
                }
            },
            MemoryType::LOADER_DATA => {
                let mut page: Page = Page::containing_address(VirtAddr::new(entry.phys_start));
                let mut frame: PhysFrame = PhysFrame::containing_address(PhysAddr::new(entry.phys_start));

                for i in 0..entry.page_count {
                    page += i;
                    frame += i;
                    write_serial("d");
                    unsafe {
                        page_table.map_to(page, frame, PageTableFlags::PRESENT|PageTableFlags::NO_EXECUTE|PageTableFlags::WRITABLE, allocator).unwrap().ignore();
                    }
                }
            },
            _ => {},
        }
    }
}

fn map_stack<M, A>(allocator: &mut A, page_table: &mut M, boot_services: &BootServices) -> u64 where M: MapperAllSizes, A: FrameAllocator<Size4KiB> {
    let num_frames = 20;
    let stack_base_phys = boot_services.allocate_pages(AllocateType::AnyPages, MemoryType::RUNTIME_SERVICES_DATA, num_frames).expect_success("Could not alloc stack");

    let stack_base: Page = Page::containing_address(VirtAddr::new(0x800000000));
    let stack_top: Page = Page::containing_address(VirtAddr::new(0x800000000u64 + num_frames as u64 * 0x1000u64 - 1u64));

    for page in Page::range_inclusive(stack_base, stack_top) {
        let i = page-stack_base;
        let frame: PhysFrame = PhysFrame::containing_address(PhysAddr::new(stack_base_phys + i*0x1000));
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            page_table.map_to(page, frame, flags, allocator).expect("Could not map stack").ignore();
        }
    }

    stack_top.start_address().as_u64()
}

fn context_switch(page_table: &mut OffsetPageTable, entry: u64, stack: u64) -> ! {
    let pt = page_table.level_4_table() as *const PageTable as u64;
    unsafe {
        asm!("mov cr3, {}; mov rsp, {}; push 0; jmp {}", 
        in(reg) pt,
        in(reg) stack,
        in(reg) entry);
    }

    loop {
        write_serial("after context switch ");
    }

    unreachable!();
}

fn write_serial(word: &str) {
    for chr in word.as_bytes() {
        unsafe {
            x86_64::instructions::port::PortWrite::write_to_port(0x3f8, *chr);
        }
    }
}

#[entry]
fn efi_main(image: Handle, st: SystemTable<Boot>) -> Status {
    uefi_services::init(&st).expect_success("Failed to initialize utils");

    info!("Hello, World!");

    let mut allocator = allocator::BootFrameAllocator::new(st.boot_services(), 64);
    let mut kernel_page_table = allocate_kernel_page_table(st.boot_services());

    // Load kernel
    let entry = {
        let kernel = load_file(image, &st, "kernel.elf");
        info!("{:X?}", &kernel.as_slice()[0..4]);

        let entry = load_kernel(kernel.as_slice(), st.boot_services(), &mut kernel_page_table, &mut allocator).expect("Could not load kernel");

        kernel.free(&st);

        entry
    };

    let stack = {
        map_stack(&mut allocator, &mut kernel_page_table, st.boot_services())
    };

    {
        use core::mem;
        use uefi::table::boot::MemoryDescriptor;
        use core::slice;

        let mmap_storage = {
            let max_mmap_size =
                st.boot_services().memory_map_size() + 8 * mem::size_of::<MemoryDescriptor>();
            let ptr = st
                .boot_services()
                .allocate_pool(MemoryType::LOADER_DATA, max_mmap_size)?
                .log();
            unsafe { slice::from_raw_parts_mut(ptr, max_mmap_size) }
        };

        let (_system_table, memory_map) = st.exit_boot_services(image, mmap_storage).expect_success("Failed to exit boot services");

        write_serial("before map_loader ");
        map_loader(memory_map, &mut allocator, &mut kernel_page_table);
        write_serial("after map_loader ");
    }

    context_switch(&mut kernel_page_table, entry, stack);

    // info!("Goodbye, World!");

    loop {}
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
    use uefi::{
        proto::{
            loaded_image::LoadedImage,
            media::{
                file::{File, FileAttribute, FileInfo, FileMode, FileType},
                fs::SimpleFileSystem,
            },
        }
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
        .allocate_pool(MemoryType::RUNTIME_SERVICES_DATA, info.file_size() as usize)
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
