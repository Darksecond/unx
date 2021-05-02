use bootinfo::boot_info::{ConsoleFont, FrameBuffer};
use spinning_top::{Spinlock, const_spinlock};
use uart_16550::SerialPort;
use core::{fmt, panic::PanicInfo};

use self::framebuffer::FrameBufferWriter;

mod framebuffer;

const COM1: u16 = 0x3F8;

static FRAMEBUFFER_WRITER: Spinlock<Option<FrameBufferWriter>> = const_spinlock(None);
static SERIAL_WRITER: Spinlock<SerialPort> = const_spinlock(unsafe { SerialPort::new(COM1) });

pub fn init(frame_buffer: FrameBuffer, font: ConsoleFont) {
    // Initialize FrameBufferWriter
    let mut fb_writer = FRAMEBUFFER_WRITER.lock();
    fb_writer.insert(FrameBufferWriter::new(frame_buffer, font));

    if let Some(fb_writer) = fb_writer.as_mut() {
        fb_writer.clear();
    }

    // Initialize SerialWriter
    SERIAL_WRITER.lock().init();
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use fmt::Write;

    //TODO Probably disable interrupts whilst locking; to prevent deadlocks
    
    if let Some(writer) = FRAMEBUFFER_WRITER.lock().as_mut() {
        writer.write_fmt(args).unwrap()
    }

    SERIAL_WRITER.lock().write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::console::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => (print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Force unlock the mutex; in case it was locked.
    // This is OK since we're panicking.
    unsafe {
        FRAMEBUFFER_WRITER.force_unlock();
        SERIAL_WRITER.force_unlock();
    }

    println!("{}", info);
    
    loop { core::hint::spin_loop(); }
}