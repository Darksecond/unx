use log::info;
use uefi::{Handle, ResultExt, table::{Boot, SystemTable}};

use crate::load_kernel::BOOTLOADER_DATA;

pub struct LoadedFileBuffer {
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

pub fn load_file(image: Handle, st: &SystemTable<Boot>, path: &str) -> LoadedFileBuffer {
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
