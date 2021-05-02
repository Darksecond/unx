#![no_std]
#![no_main]
#![feature(asm)]

mod console;

use core::u8;

use bootinfo::boot_info::BootInfo;

pub struct SerialWriter;

impl core::fmt::Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        write_serial(s);
        Ok(())
    }
}

//TODO We probably want to wrap this into bootinfo and into a macro.
//TODO This _will_ output a symbol into the kernel executable elf
// #[used]
// #[link_section = ".notes"]
// #[no_mangle]
// pub static STACK_BASE: u64 = 0xDEADBEEF;

unsafe fn wait_serial() {
    let mut value: u8 = x86_64::instructions::port::PortRead::read_from_port(0x3f8 + 5);
    while value & 0x20 == 0 {
        value = x86_64::instructions::port::PortRead::read_from_port(0x3f8 + 5);
    }
}

fn write_serial(word: &str) {
    for chr in word.as_bytes() {
        unsafe {
            wait_serial();
            x86_64::instructions::port::PortWrite::write_to_port(0x3f8, *chr);
        }
    }
}

#[no_mangle]
pub extern "C" fn _start(boot_info: &'static mut BootInfo) -> ! {
    console::init(boot_info.frame_buffer, boot_info.console_font);

    println!("Hello, World!");

    loop {}
}