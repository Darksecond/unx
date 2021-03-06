#![no_std]
#![no_main]
#![feature(asm)]
#![feature(alloc_error_handler)]
#![feature(const_mut_refs)] // For fixed_size_block

extern crate alloc;

mod console;
mod memory;

use bootinfo::boot_info::BootInfo;

//TODO We probably want to wrap this into bootinfo and into a macro.
//TODO This _will_ output a symbol into the kernel executable elf
// #[used]
// #[link_section = ".notes"]
// #[no_mangle]
// pub static STACK_BASE: u64 = 0xDEADBEEF;

#[no_mangle]
pub extern "C" fn _start(boot_info: &'static mut BootInfo) -> ! {
    console::init(boot_info.frame_buffer, boot_info.console_font);
    memory::init(&boot_info.memory_map);

    let x = alloc::boxed::Box::new(5);

    println!("Hello, World! {}", x);

    loop {}
}