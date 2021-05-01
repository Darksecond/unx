#![no_std]
#![no_main]
#![feature(asm)]

use core::{panic::PanicInfo, u8};

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

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    use core::fmt::Write;
    writeln!(SerialWriter, "{}", info).unwrap();
    loop {}
}

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
    use core::fmt::Write;

    write_serial("\n\n");
    write_serial("Hello, Kernel!\n");
    write_serial("This is being printed from the kernel!\n");

    {
        let font = boot_info.console_font;
        let psf = psf::Font::new(font.as_slice());
        let char = psf.glyph('&').unwrap();

        let mut frame_buffer = boot_info.frame_buffer;
        let stride = frame_buffer.info.stride as u32;
        let buffer = frame_buffer.buffer_mut();

        for y in 0..char.height() {
            for x in 0..char.width() {
                let bit = char.pixel(x,y).unwrap();
                let color = if bit { 255 } else { 0 };
                
                buffer[(y*stride*4+x*4+32) as usize] = color;
            }
        }
    }

    // let framebuffer: &mut FrameBuffer = &mut boot_info.frame_buffer;
    
    // let buffer: &mut [u8] = unsafe {
    //     core::slice::from_raw_parts_mut((PHYSMAP_BASE + 0xc0000000) as *mut u8, framebuffer.buffer_size)
    // };

    

    for entry in boot_info.memory_map.entries() {
        writeln!(SerialWriter, "{:?}", entry).unwrap();
    }
    
    // for i in 0..framebuffer.info().width*framebuffer.info().height {
    //     let buffer = framebuffer.buffer_mut();

    //     buffer[4*i+0] = 255;
    //     buffer[4*i+1] = 0;
    //     buffer[4*i+2] = 255;
    // }

    loop {}
}