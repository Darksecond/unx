#![no_std]

use byteorder::{LittleEndian, ByteOrder};

//TODO support unicode tables

#[derive(Debug)]
pub enum FontError {
    InvalidMagic,
    UnknownVersion,
    InvalidHeaderSize,
}

#[derive(Debug)]
struct FontInfo {
    width: u32,
    height: u32,
    bytes_per_glyph: u32,
    num_glyphs: u32,
}

impl FontInfo {
    pub fn new(data: &[u8]) -> Result<Self, FontError> {
        let magic = LittleEndian::read_u32(&data[0..4]);
        let version = LittleEndian::read_u32(&data[4..8]);
        let header_size = LittleEndian::read_u32(&data[8..12]);
        let _flags = LittleEndian::read_u32(&data[12..16]);
        let num_glyphs = LittleEndian::read_u32(&data[16..20]);
        let bytes_per_glyph = LittleEndian::read_u32(&data[20..24]);
        let height = LittleEndian::read_u32(&data[24..28]);
        let width = LittleEndian::read_u32(&data[28..32]);

        if magic != 0x864ab572 { return Err(FontError::InvalidMagic); }
        if version != 0 { return Err(FontError::UnknownVersion); }
        if header_size != 32 { return Err(FontError::InvalidHeaderSize); }

        Ok(FontInfo {
            width,
            height,
            bytes_per_glyph,
            num_glyphs,
        })
    }
}

#[derive(Debug)]
pub struct Font<D> where D: AsRef<[u8]> {
    info: FontInfo,
    data: D,
}

#[derive(Debug)]
pub struct Glyph<'a> {
    width: u32,
    height: u32,
    stride: u32,
    data: &'a [u8],
}

impl<'a> Glyph<'a> {
    pub fn pixel(&self, x: u32, y: u32) -> Option<bool> {
        if x > self.width || y > self.height {
            return None;
        }

        let byte = self.data[(y * self.stride + x / 8) as usize];
        let bit = byte >> (7 - (x % 8)) & 0b01 != 0;

        Some(bit)
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn width(&self) -> u32 {
        self.width
    }
}

impl<D> Font<D> where D: AsRef<[u8]> {
    /// This is implemented for PSF version 2.
    pub fn new(data: D) -> Result<Self, FontError> {
        Ok(Font {
            info: FontInfo::new(data.as_ref())?,
            data,
        })
    }

    pub fn width(&self) -> u32 {
        self.info.width
    }

    pub fn height(&self) -> u32 {
        self.info.height
    }

    pub fn glyph(&self, character: char) -> Option<Glyph> {
        let character = character as usize;
        if character > self.info.num_glyphs as _ { 
            return None;
        }

        let bytes_per_glyph = self.info.bytes_per_glyph as usize;

        Some(Glyph {
            width: self.info.width,
            height: self.info.height,
            stride: (self.info.width + 7)/8,
            data: &self.data.as_ref()[(32 + character * bytes_per_glyph)..(32 + character * bytes_per_glyph + bytes_per_glyph)],
        })
    }
}