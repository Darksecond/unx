#![no_std]
#![no_main]
#![feature(asm)]

use core::panic::PanicInfo;

//TODO We probably want to wrap this into bootinfo and into a macro.
//TODO This _will_ output a symbol into the kernel executable elf
// #[used]
// #[link_section = ".notes"]
// #[no_mangle]
// pub static STACK_BASE: u64 = 0xDEADBEEF;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

fn write_serial(word: &str) {
    for chr in word.as_bytes() {
        unsafe {
            x86_64::instructions::port::PortWrite::write_to_port(0x3f8, *chr);
        }
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    write_serial("\n\n");
    write_serial("Hello, Kernel!\n");
    write_serial("This is being printed from the kernel!\n");
    loop {}
}