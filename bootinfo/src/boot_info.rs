use core::slice;


#[repr(C)]
#[derive(Debug, Default)]
pub struct BootInfo {
    pub frame_buffer: FrameBuffer,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct FrameBuffer {
    pub buffer_base: u64,
    pub buffer_size: usize,
    pub info: FrameBufferInfo,
}

impl FrameBuffer {
    pub fn buffer(&self) -> &[u8] {
        unsafe { self.create_buffer() }
    }

    pub fn buffer_mut(&mut self) -> &mut [u8] {
        unsafe { self.create_buffer() }
    }

    unsafe fn create_buffer<'a>(&self) -> &'a mut [u8] {
        slice::from_raw_parts_mut(self.buffer_base as *mut u8, self.buffer_size)
    }

    pub fn info(&self) -> FrameBufferInfo {
        self.info
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FrameBufferInfo {
    pub width: usize,
    pub height: usize,
    pub stride: usize,
}