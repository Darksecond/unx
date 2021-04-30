use core::slice;

const MAX_MEMORY_MAP_ENTRIES: usize = 256;

#[derive(Debug)]
pub struct MemoryMap {
    num_entries: usize,
    entries: [MemoryMapEntry; MAX_MEMORY_MAP_ENTRIES],
}

impl MemoryMap {
    pub fn add_entry(&mut self, entry: MemoryMapEntry) -> Result<(), ()> {
        if self.num_entries >= MAX_MEMORY_MAP_ENTRIES {
            return Err(());
        }

        self.entries[self.num_entries as usize] = entry;
        self.num_entries += 1;

        Ok(())
    }
    
    pub fn entries(&self) -> &[MemoryMapEntry] {
        &self.entries[0..(self.num_entries)]
    }
}

impl Default for MemoryMap {
    fn default() -> Self {
        MemoryMap {
            num_entries: 0,
            entries: [Default::default(); MAX_MEMORY_MAP_ENTRIES],
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum MemoryType {
    Unusable,
    Conventional,
}

#[derive(Debug, Clone, Copy)]
pub struct MemoryMapEntry {
    pub start: u64,
    pub size: usize,
    pub memory_type: MemoryType,
}

impl Default for MemoryMapEntry {
    fn default() -> Self {
        MemoryMapEntry {
            start: 0,
            size: 0,
            memory_type: MemoryType::Unusable,
        }
    }
}


#[derive(Debug, Default)]
pub struct BootInfo {
    pub frame_buffer: FrameBuffer,
    pub memory_map: MemoryMap,

    pub console_font_base: u64,
    pub console_font_size: usize,
}

impl BootInfo {
    pub fn console_font(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.console_font_base as *const u8, self.console_font_size) }
    }
}

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