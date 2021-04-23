#![no_std]
#![no_main]
#![feature(asm)]
#![feature(abi_efiapi)]

use uefi::{ResultExt, prelude::{entry, Boot, Handle, Status, SystemTable}};
use log::info;

#[entry]
fn efi_main(_image: Handle, st: SystemTable<Boot>) -> Status {
    uefi_services::init(&st).expect_success("Failed to initialize utils");

    info!("Hello, World!");
    
    Status::SUCCESS
}