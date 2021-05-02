use core::{fmt, panic::PanicInfo};
use bootinfo::boot_info::{ConsoleFont, FrameBuffer};
use spinning_top::{Spinlock, const_spinlock};

const BYTES_PER_PIXEL: usize = 4;

struct Color {
    r: u8,
    g: u8,
    b: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self {
            r,
            g,
            b,
        }
    }

    pub fn black() -> Self {
        Color::new(0, 0, 0)
    }

    pub fn white() -> Self {
        Color::new(255, 255, 255)
    }
}

fn draw_pixel(mut frame_buffer: FrameBuffer, x: usize, y: usize, color: Color) {
    let info = frame_buffer.info();
    let buffer = frame_buffer.buffer_mut();

    if x >= info.width || y >= info.height {
        return;
    }

    let index = y * info.stride * BYTES_PER_PIXEL + x * BYTES_PER_PIXEL;

    //TODO Volatile
    buffer[index+0] = color.b;
    buffer[index+1] = color.g;
    buffer[index+2] = color.r;
}

struct FrameBufferWriter {
    frame_buffer: FrameBuffer,
    font: psf::Font<ConsoleFont>,
    x: usize,
    y: usize,
}

impl FrameBufferWriter {
    fn clear(&mut self) {
        let info = self.frame_buffer.info();

        for y in 0..info.height {
            for x in 0..info.width {
                draw_pixel(self.frame_buffer, x, y, Color::black());
            }
        }

        // Reset x and y positions
        self.x = 0;
        self.y = 0;
    }

    pub fn draw_char(&mut self, character: char) {
        if let Some(glyph) = self.font.glyph(character) {
            for y in 0..glyph.height() {
                for x in 0..glyph.width() {
                    let pixel = glyph.pixel(x, y).unwrap_or(false);
                    let pixel = if pixel { Color::white() } else { Color::black() };
                    draw_pixel(self.frame_buffer, self.x + x as usize, self.y + y as usize, pixel);
                }
            }
        }
    }

    fn scroll_line(&mut self) {
        let info = self.frame_buffer.info();
        let buffer = self.frame_buffer.buffer_mut();
        let font_height = self.font.height() as usize;

        // Scroll screen
        {
            let count = info.stride * (info.height - font_height) * BYTES_PER_PIXEL;
            let dst = buffer.as_mut_ptr();
            let src = buffer.as_ptr().wrapping_offset((info.stride*font_height*BYTES_PER_PIXEL) as _);
            unsafe {
                core::ptr::copy(src, dst, count);
            }
        }

        // Clear now scrolled section
        {
            unsafe {
                let offset = info.stride * (info.height - font_height) * BYTES_PER_PIXEL;
                let total = info.stride * info.height * BYTES_PER_PIXEL;
                let dst = buffer.as_mut_ptr().wrapping_offset(offset as _);
                let count = total - offset;

                core::ptr::write_bytes(dst, 0, count)
            }
        }
    }

    fn new_line(&mut self) {
        let info = self.frame_buffer.info();
        let font_height = self.font.height() as usize;
        let max_height = (info.height / font_height) * font_height;

        self.y += font_height;
        self.x = 0;

        if self.y >= max_height {
            self.scroll_line();
            self.y = max_height - font_height;
        }
    }

    pub fn write_char(&mut self, character: char) {
        let info = self.frame_buffer.info();

        match character {
            '\n' => {
                self.new_line();
            },
            character => {
                self.draw_char(character);
                self.x += self.font.width() as usize;

                if self.x >= info.width {
                    self.new_line();
                }
            },
        }
    }
}

impl fmt::Write for FrameBufferWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for character in s.chars() {
            self.write_char(character);
        }

        Ok(())
    }
}

static FRAMEBUFFER_WRITER: Spinlock<Option<FrameBufferWriter>> = const_spinlock(None);

pub fn init(frame_buffer: FrameBuffer, font: ConsoleFont) {
    let mut guard = FRAMEBUFFER_WRITER.lock();
    guard.insert(FrameBufferWriter {
        frame_buffer,
        font: psf::Font::new(font).unwrap(),
        x: 0,
        y: 0,
    });

    if let Some(writer) = guard.as_mut() {
        writer.clear();
    }
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use fmt::Write;

    //TODO Probably disable interrupts whilst locking; to prevent deadlocks

    //TODO Also output to SerialWriter
    if let Some(writer) = FRAMEBUFFER_WRITER.lock().as_mut() {
        writer.write_fmt(args).unwrap()
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::console::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => (print!("\n"));
    ($($arg:tt)*) => (print!("{}\n", format_args!($($arg)*)));
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Force unlock the mutex; in case it was locked.
    // This is OK since we're panicking.
    unsafe {
        FRAMEBUFFER_WRITER.force_unlock();
    }

    println!("{}", info);
    
    loop { x86_64::instructions::hlt(); }
}