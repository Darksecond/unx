#![no_std]
#![no_main]
#![feature(asm)]

use core::panic::PanicInfo;

use bootinfo::boot_info::BootInfo;

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
pub extern "C" fn _start(boot_info: &'static mut BootInfo) -> ! {
    write_serial("\n\n");
    write_serial("Hello, Kernel!\n");
    write_serial("This is being printed from the kernel!\n");

    let framebuffer = &mut boot_info.frame_buffer;
    
    // let buffer: &mut [u8] = unsafe {
    //     core::slice::from_raw_parts_mut((PHYSMAP_BASE + 0xc0000000) as *mut u8, framebuffer.buffer_size)
    // };
    
    for i in 0..framebuffer.info().width*framebuffer.info().height {
        let buffer = framebuffer.buffer_mut();

        buffer[4*i+0] = 255;
        buffer[4*i+1] = 0;
        buffer[4*i+2] = 255;
    }

    loop {}
}