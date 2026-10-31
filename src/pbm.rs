use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PbmImage {
    pub width: usize,
    pub height: usize,
    pub line_size: usize, // (width + 7) / 8
    pub data: Vec<u8>,
}

impl PbmImage {
    pub fn new(width: usize, height: usize) -> Self {
        let line_size = width.div_ceil(8);
        Self {
            width,
            height,
            line_size,
            data: vec![0u8; line_size * height],
        }
    }

    pub fn line(&self, y: usize) -> &[u8] {
        let start = y * self.line_size;
        &self.data[start..start + self.line_size]
    }

    pub fn line_mut(&mut self, y: usize) -> &mut [u8] {
        let start = y * self.line_size;
        &mut self.data[start..start + self.line_size]
    }

    #[allow(dead_code)]
    pub fn get_pixel(&self, x: usize, y: usize) -> bool {
        if x < self.width && y < self.height {
            let byte_idx = y * self.line_size + (x / 8);
            let bit_idx = 7 - (x % 8);
            (self.data[byte_idx] & (1 << bit_idx)) != 0
        } else {
            false
        }
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, black: bool) {
        if x < self.width && y < self.height {
            let byte_idx = y * self.line_size + (x / 8);
            let bit_idx = 7 - (x % 8);
            if black {
                self.data[byte_idx] |= 1 << bit_idx;
            } else {
                self.data[byte_idx] &= !(1 << bit_idx);
            }
        }
    }

    pub fn read_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        Self::read(&mut reader)
    }

    pub fn read<R: Read>(reader: &mut BufReader<R>) -> io::Result<Self> {
        let mut magic = String::new();
        // Read magic
        loop {
            magic.clear();
            reader.read_line(&mut magic)?;
            let trimmed = magic.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed != "P4" {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Expected binary PBM header 'P4', got '{}'", trimmed),
                ));
            }
            break;
        }

        // Read dimensions (skipping comments)
        let mut dims = Vec::new();
        while dims.len() < 2 {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "EOF while reading PBM dimensions",
                ));
            }
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            for token in trimmed.split_whitespace() {
                if let Ok(val) = token.parse::<usize>() {
                    dims.push(val);
                }
            }
        }

        let width = dims[0];
        let height = dims[1];
        let line_size = width.div_ceil(8);
        let mut data = vec![0u8; line_size * height];
        reader.read_exact(&mut data)?;

        Ok(Self {
            width,
            height,
            line_size,
            data,
        })
    }

    pub fn write_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let mut file = File::create(path)?;
        let header = format!(
            "P4\n# Created by CAPT-PoC\n{} {}\n",
            self.width, self.height
        );
        file.write_all(header.as_bytes())?;
        file.write_all(&self.data)?;
        file.flush()?;
        Ok(())
    }

    pub fn write_bmp<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let mut file = File::create(path)?;
        let row_bytes = self.width.div_ceil(32) * 4;
        let image_size = row_bytes * self.height;
        let file_size = 14 + 40 + 8 + image_size;

        let mut header = [0u8; 62];
        // BITMAPFILEHEADER (14 bytes)
        header[0] = b'B';
        header[1] = b'M';
        header[2..6].copy_from_slice(&(file_size as u32).to_le_bytes());
        header[10..14].copy_from_slice(&62u32.to_le_bytes()); // Pixel data offset

        // BITMAPINFOHEADER (40 bytes)
        header[14..18].copy_from_slice(&40u32.to_le_bytes()); // Header size
        header[18..22].copy_from_slice(&(self.width as i32).to_le_bytes());
        header[22..26].copy_from_slice(&(self.height as i32).to_le_bytes()); // Positive height = bottom-up
        header[26..28].copy_from_slice(&1u16.to_le_bytes()); // Planes
        header[28..30].copy_from_slice(&1u16.to_le_bytes()); // 1 bpp
        header[30..34].copy_from_slice(&0u32.to_le_bytes()); // BI_RGB (uncompressed)
        header[34..38].copy_from_slice(&(image_size as u32).to_le_bytes());
        header[38..42].copy_from_slice(&23622u32.to_le_bytes()); // 600 DPI = ~23622 px/m
        header[42..46].copy_from_slice(&23622u32.to_le_bytes());
        header[46..50].copy_from_slice(&2u32.to_le_bytes()); // 2 colors in palette
        header[50..54].copy_from_slice(&2u32.to_le_bytes());

        // Color table: Index 0 = White, Index 1 = Black
        header[54] = 255;
        header[55] = 255;
        header[56] = 255;
        header[57] = 0;
        header[58] = 0;
        header[59] = 0;
        header[60] = 0;
        header[61] = 0;

        file.write_all(&header)?;

        let pad_len = row_bytes.saturating_sub(self.line_size);
        let pad = vec![0u8; pad_len];
        for y in (0..self.height).rev() {
            file.write_all(self.line(y))?;
            if !pad.is_empty() {
                file.write_all(&pad)?;
            }
        }
        file.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_pbm_new_and_pixel_access() {
        let mut img = PbmImage::new(16, 2);
        assert_eq!(img.width, 16);
        assert_eq!(img.height, 2);
        assert_eq!(img.line_size, 2);
        assert_eq!(img.data.len(), 4);

        // All initially 0 (white)
        assert_eq!(img.data, vec![0, 0, 0, 0]);

        // Set pixel (0, 0)
        img.set_pixel(0, 0, true);
        assert_eq!(img.line(0)[0], 0x80);

        // Set pixel (7, 0)
        img.set_pixel(7, 0, true);
        assert_eq!(img.line(0)[0], 0x81);

        // Set pixel (8, 0)
        img.set_pixel(8, 0, true);
        assert_eq!(img.line(0)[1], 0x80);

        // Clear pixel (0, 0)
        img.set_pixel(0, 0, false);
        assert_eq!(img.line(0)[0], 0x01);

        // Out of bounds should safely do nothing
        img.set_pixel(100, 100, true);
    }

    #[test]
    fn test_pbm_read_with_comments() {
        let pbm_raw = b"P4\n# Comment 1\n# Comment 2\n16 2\n\x81\x80\x00\xFF";
        let mut reader = BufReader::new(Cursor::new(pbm_raw));
        let img = PbmImage::read(&mut reader).unwrap();

        assert_eq!(img.width, 16);
        assert_eq!(img.height, 2);
        assert_eq!(img.line_size, 2);
        assert_eq!(img.data, vec![0x81, 0x80, 0x00, 0xFF]);
    }

    #[test]
    fn test_pbm_invalid_magic() {
        let bad = b"P1\n16 2\n0 1";
        let mut reader = BufReader::new(Cursor::new(bad));
        assert!(PbmImage::read(&mut reader).is_err());
    }

    #[test]
    fn test_pbm_truncated_data() {
        let truncated = b"P4\n16 2\n\x81\x80"; // expected 4 data bytes, only 2
        let mut reader = BufReader::new(Cursor::new(truncated));
        assert!(PbmImage::read(&mut reader).is_err());
    }
}
