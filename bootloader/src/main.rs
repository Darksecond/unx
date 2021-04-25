#![no_std]
#![no_main]
#![feature(asm)]
#![feature(abi_efiapi)]

use log::info;
use uefi::{
    prelude::{entry, Boot, Handle, Status, SystemTable},
    ResultExt,
};

#[entry]
fn efi_main(image: Handle, st: SystemTable<Boot>) -> Status {
    uefi_services::init(&st).expect_success("Failed to initialize utils");

    info!("Hello, World!");

    // Load kernel
    {
        let buffer = load_file(image, &st, "kernel.elf");
        info!("{:X?}", &buffer[0..4]);
    }

    loop {}
}

fn load_file<'a>(image: Handle, st: &SystemTable<Boot>, path: &str) -> &'a mut [u8] {
    use uefi::{
        proto::{
            loaded_image::LoadedImage,
            media::{
                file::{File, FileAttribute, FileInfo, FileMode, FileType},
                fs::SimpleFileSystem,
            },
        },
        table::boot::MemoryType,
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
        .allocate_pool(MemoryType::LOADER_DATA, info.file_size() as usize)
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

    buffer
}