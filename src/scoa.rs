#![allow(dead_code)]
use std::io::{self, Read};

pub const REPEAT_X_BYTE: u8 = 0x43;
pub const OP_NOP: u8 = 0x40;
pub const OP_EOL: u8 = 0x41;
pub const OP_EOP: u8 = 0x42;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoaFuncType {
    Copy,
    Repeat,
    Raw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoaFunc {
    pub func_type: ScoaFuncType,
    pub index: usize,
    pub count: usize,
}

impl ScoaFunc {
    pub fn payload<'a>(&self, line: &'a [u8]) -> &'a [u8] {
        &line[self.index..self.index + self.count]
    }
}

pub struct ScoaState<'a> {
    line: &'a [u8],
    prev_line: Option<&'a [u8]>,
    index: usize,
    curr: Option<ScoaFunc>,
    last: Option<ScoaFunc>,
}

impl<'a> ScoaState<'a> {
    pub fn new(line: &'a [u8], prev_line: Option<&'a [u8]>) -> Self {
        let mut state = Self {
            line,
            prev_line,
            index: 0,
            curr: None,
            last: None,
        };
        state.advance();
        state
    }

    fn next_func(&self, idx: usize, last: Option<&ScoaFunc>) -> Option<ScoaFunc> {
        let prev = self.prev_line;
        if idx >= 1
            && self.line[idx] == self.line[idx - 1]
            && last.is_none_or(|l| l.func_type != ScoaFuncType::Copy)
        {
            if let Some(l) = last
                && (l.func_type == ScoaFuncType::Repeat || l.count == 1)
            {
                return None;
            }
            return Some(ScoaFunc {
                func_type: ScoaFuncType::Repeat,
                index: idx - 1,
                count: 2,
            });
        } else if let Some(prev_bytes) = prev
            && idx < prev_bytes.len()
            && self.line[idx] == prev_bytes[idx]
        {
            if let Some(l) = last
                && l.func_type == ScoaFuncType::Copy
            {
                return None;
            }
            return Some(ScoaFunc {
                func_type: ScoaFuncType::Copy,
                index: idx,
                count: 1,
            });
        }
        if let Some(l) = last
            && l.func_type == ScoaFuncType::Raw
        {
            return None;
        }
        Some(ScoaFunc {
            func_type: ScoaFuncType::Raw,
            index: idx,
            count: 1,
        })
    }

    pub fn peek(&self) -> Option<ScoaFunc> {
        self.curr
    }

    pub fn get(&mut self) -> Option<ScoaFunc> {
        let res = self.curr;
        self.advance();
        res
    }

    pub fn is_empty(&self) -> bool {
        self.curr.is_none()
    }

    fn advance(&mut self) -> bool {
        while self.index < self.line.len() {
            let idx = self.index;
            let next = if let Some(ref mut l) = self.last {
                if idx >= 1
                    && self.line[idx] == self.line[idx - 1]
                    && l.func_type != ScoaFuncType::Copy
                {
                    if l.func_type == ScoaFuncType::Repeat {
                        l.count += 1;
                        None
                    } else if l.count == 1 {
                        l.func_type = ScoaFuncType::Repeat;
                        l.count = 2;
                        None
                    } else {
                        l.count -= 1;
                        Some(ScoaFunc {
                            func_type: ScoaFuncType::Repeat,
                            index: idx - 1,
                            count: 2,
                        })
                    }
                } else if self
                    .prev_line
                    .is_some_and(|p| idx < p.len() && self.line[idx] == p[idx])
                {
                    if l.func_type == ScoaFuncType::Copy {
                        l.count += 1;
                        None
                    } else {
                        Some(ScoaFunc {
                            func_type: ScoaFuncType::Copy,
                            index: idx,
                            count: 1,
                        })
                    }
                } else if l.func_type == ScoaFuncType::Raw {
                    l.count += 1;
                    None
                } else {
                    Some(ScoaFunc {
                        func_type: ScoaFuncType::Raw,
                        index: idx,
                        count: 1,
                    })
                }
            } else {
                self.next_func(idx, None)
            };

            self.index += 1;
            if let Some(n) = next {
                if self.index > 1 {
                    self.curr = self.last;
                    self.last = Some(n);
                    return true;
                }
                self.last = Some(n);
            }
        }

        if self.last.is_some() {
            self.curr = self.last.take();
            return true;
        }

        self.curr = None;
        false
    }
}

pub struct ScoaEncoder {
    pub video_data: Vec<u8>,
    pub line_size: usize,
    prev_line: Option<Vec<u8>>,
}

impl ScoaEncoder {
    pub fn new(line_size: usize) -> Self {
        Self {
            video_data: Vec::with_capacity(32768),
            line_size,
            prev_line: None,
        }
    }

    fn extend(&mut self, count: usize) {
        assert!((8..=255).contains(&count));
        self.video_data.push(0xa0 | (((count >> 3) & 0x1f) as u8));
    }

    fn copy_long(&mut self, copy_count: usize) {
        assert!((8..=248).contains(&copy_count) && copy_count.is_multiple_of(8));
        self.video_data
            .push(0x80 | (((copy_count >> 3) & 0x1f) as u8));
    }

    fn copy_short(&mut self, copy_count: usize) {
        assert!((1..=7).contains(&copy_count));
        self.video_data.push(0xc0 | (copy_count as u8 & 7));
    }

    fn copy_then_raw(&mut self, copy_count: usize, raw: &[u8]) {
        assert!(copy_count <= 7);
        assert!(!raw.is_empty() && raw.len() <= 7);
        self.video_data
            .push((((raw.len() & 7) as u8) << 3) | (copy_count as u8 & 7));
        self.video_data.extend_from_slice(raw);
    }

    fn copy_then_raw_long(&mut self, copy_count: usize, raw: &[u8]) {
        assert!(copy_count <= 7);
        assert!(raw.len() >= 8 && raw.len() <= 255);
        self.extend(raw.len());
        self.video_data
            .push(0xc0 | (((raw.len() & 7) as u8) << 3) | (copy_count as u8 & 7));
        self.video_data.extend_from_slice(raw);
    }

    fn copy_then_repeat(&mut self, copy_count: usize, repeat_count: usize, repeat_byte: u8) {
        assert!(copy_count <= 7);
        assert!((2..=7).contains(&repeat_count));
        self.video_data
            .push(0x40 | (((repeat_count & 7) as u8) << 3) | (copy_count as u8 & 7));
        self.video_data.push(repeat_byte);
    }

    fn copy_then_repeat_long(&mut self, copy_count: usize, repeat_count: usize, repeat_byte: u8) {
        assert!(copy_count <= 7);
        assert!((8..=255).contains(&repeat_count));
        self.extend(repeat_count);
        self.video_data
            .push(0x80 | (((repeat_count & 7) as u8) << 3) | (copy_count as u8 & 7));
        self.video_data.push(repeat_byte);
    }

    fn repeat_then_raw(&mut self, repeat_count: usize, repeat_byte: u8, raw: &[u8]) {
        assert!((2..=7).contains(&repeat_count));
        assert!(!raw.is_empty() && raw.len() <= 7);
        self.video_data
            .push(0xc0 | (((repeat_count & 7) as u8) << 3) | (raw.len() as u8 & 7));
        self.video_data.push(repeat_byte);
        self.video_data.extend_from_slice(raw);
    }

    fn repeat_then_raw_long(&mut self, repeat_count: usize, repeat_byte: u8, raw: &[u8]) {
        assert!((2..=7).contains(&repeat_count));
        assert!(raw.len() >= 8 && raw.len() <= 255);
        self.extend(raw.len());
        self.video_data
            .push(0x40 | (((repeat_count & 7) as u8) << 3) | (raw.len() as u8 & 7));
        self.video_data.push(repeat_byte);
        self.video_data.extend_from_slice(raw);
    }

    fn repeat_x(&mut self, repeat_count: usize) {
        assert!((1..=7).contains(&repeat_count));
        self.video_data
            .push(0xc0 | (((repeat_count & 7) as u8) << 3));
    }

    fn repeat_x_long(&mut self, repeat_count: usize) {
        assert!((8..=255).contains(&repeat_count));
        self.extend(repeat_count);
        self.video_data.push(((repeat_count & 7) as u8) << 3);
    }

    fn emit_copy(&mut self, mut copy_count: usize) {
        while copy_count >= 8 {
            let mut copy = copy_count.min(248);
            copy -= copy % 8;
            self.copy_long(copy);
            copy_count -= copy;
        }
        if copy_count != 0 {
            self.copy_short(copy_count);
        }
    }

    fn emit_copy_then_raw(&mut self, mut copy_count: usize, mut raw: &[u8]) {
        if copy_count <= 7 {
            while raw.len() >= 8 {
                let count = raw.len().min(255);
                self.copy_then_raw_long(copy_count, &raw[..count]);
                copy_count = 0;
                raw = &raw[count..];
            }
            if copy_count != 0 || !raw.is_empty() {
                self.copy_then_raw(copy_count, raw);
            }
        } else {
            while copy_count >= 8 {
                let mut copy = copy_count.min(248);
                copy -= copy % 8;
                self.copy_long(copy);
                copy_count -= copy;
            }
            self.emit_copy_then_raw(copy_count, raw);
        }
    }

    fn emit_write_raw(&mut self, raw: &[u8]) {
        self.emit_copy_then_raw(0, raw);
    }

    fn emit_copy_then_repeat(
        &mut self,
        mut copy_count: usize,
        mut repeat_count: usize,
        repeat_byte: u8,
    ) {
        if copy_count <= 7 {
            while repeat_count >= 8 {
                let rep = repeat_count.min(255);
                self.copy_then_repeat_long(copy_count, rep, repeat_byte);
                copy_count = 0;
                repeat_count -= rep;
            }
            if repeat_count >= 2 {
                self.copy_then_repeat(copy_count, repeat_count, repeat_byte);
            } else if repeat_count == 1 {
                self.copy_then_raw(copy_count, &[repeat_byte]);
            } else if copy_count != 0 {
                self.copy_short(copy_count);
            }
        } else {
            while copy_count >= 8 {
                let mut copy = copy_count.min(248);
                copy -= copy % 8;
                self.copy_long(copy);
                copy_count -= copy;
            }
            self.emit_copy_then_repeat(copy_count, repeat_count, repeat_byte);
        }
    }

    fn emit_repeat_x(&mut self, mut repeat_count: usize) {
        while repeat_count >= 8 {
            let rep = repeat_count.min(255);
            self.repeat_x_long(rep);
            repeat_count -= rep;
        }
        if repeat_count != 0 {
            self.repeat_x(repeat_count);
        }
    }

    fn emit_repeat(&mut self, repeat_count: usize, repeat_byte: u8) {
        if repeat_byte == REPEAT_X_BYTE {
            self.emit_repeat_x(repeat_count);
        } else {
            self.emit_copy_then_repeat(0, repeat_count, repeat_byte);
        }
    }

    fn emit_repeat_then_raw(&mut self, mut repeat_count: usize, repeat_byte: u8, mut raw: &[u8]) {
        if repeat_count <= 7 {
            if raw.len() <= 7 {
                if raw.is_empty() {
                    self.emit_copy_then_repeat(0, repeat_count, repeat_byte);
                } else if repeat_count == 0 {
                    self.copy_then_raw(0, raw);
                } else if repeat_count >= 2 {
                    self.repeat_then_raw(repeat_count, repeat_byte, raw);
                } else {
                    self.emit_copy_then_repeat(0, repeat_count, repeat_byte);
                    self.copy_then_raw(0, raw);
                }
            } else {
                if repeat_count >= 2 {
                    let count = raw.len().min(255);
                    self.repeat_then_raw_long(repeat_count, repeat_byte, &raw[..count]);
                    raw = &raw[count..];
                } else if repeat_count != 0 {
                    self.emit_copy_then_repeat(0, repeat_count, repeat_byte);
                }
                while raw.len() >= 8 {
                    let count = raw.len().min(255);
                    self.copy_then_raw_long(0, &raw[..count]);
                    raw = &raw[count..];
                }
                if !raw.is_empty() {
                    self.copy_then_raw(0, raw);
                }
            }
        } else {
            while repeat_count >= 8 {
                let count = repeat_count.min(255);
                self.copy_then_repeat_long(0, count, repeat_byte);
                repeat_count -= count;
            }
            self.emit_repeat_then_raw(repeat_count, repeat_byte, raw);
        }
    }

    pub fn encode_line(&mut self, line: &[u8]) {
        assert_eq!(line.len(), self.line_size);

        // Pre-collect functions from ScoaState
        let mut funcs = Vec::new();
        {
            let mut state = ScoaState::new(line, self.prev_line.as_deref());
            while let Some(f) = state.get() {
                funcs.push(f);
            }
        }

        let mut i = 0;
        while i < funcs.len() {
            let curr = funcs[i];
            match curr.func_type {
                ScoaFuncType::Copy => {
                    if curr.index + curr.count == line.len() {
                        self.video_data.push(OP_EOL);
                        break;
                    }
                    if i + 1 < funcs.len() {
                        let next = funcs[i + 1];
                        if next.func_type == ScoaFuncType::Repeat {
                            i += 2;
                            self.emit_copy_then_repeat(curr.count, next.count, line[next.index]);
                            continue;
                        } else if next.func_type == ScoaFuncType::Raw {
                            i += 2;
                            self.emit_copy_then_raw(curr.count, next.payload(line));
                            continue;
                        }
                    }
                    self.emit_copy(curr.count);
                }
                ScoaFuncType::Repeat => {
                    if i + 1 < funcs.len() {
                        let next = funcs[i + 1];
                        if next.func_type == ScoaFuncType::Raw {
                            i += 2;
                            self.emit_repeat_then_raw(
                                curr.count,
                                line[curr.index],
                                next.payload(line),
                            );
                            continue;
                        }
                    }
                    self.emit_repeat(curr.count, line[curr.index]);
                }
                ScoaFuncType::Raw => {
                    self.emit_write_raw(curr.payload(line));
                }
            }
            i += 1;
        }

        self.prev_line = Some(line.to_vec());
    }

    pub fn finish(mut self) -> Vec<u8> {
        // Enforce 16-bit word alignment
        if !(self.video_data.len() + 1).is_multiple_of(2) {
            self.video_data.push(OP_NOP);
        }
        self.video_data.push(OP_EOP);
        self.video_data
    }
}

pub struct ScoaDecoder {
    pub line_size: usize,
    pub decoded_data: Vec<u8>,
    pub eop_reached: bool,
}

impl ScoaDecoder {
    pub fn new(line_size: usize) -> Self {
        Self {
            line_size,
            decoded_data: Vec::new(),
            eop_reached: false,
        }
    }

    fn copy(&mut self, count: usize) -> io::Result<()> {
        if count == 0 {
            return Ok(());
        }
        if self.decoded_data.len() < self.line_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Decoder: Copy attempted before previous line existed",
            ));
        }
        let start = self.decoded_data.len() - self.line_size;
        for i in 0..count {
            let b = self.decoded_data[start + i];
            self.decoded_data.push(b);
        }
        Ok(())
    }

    fn repeat(&mut self, count: usize, byte: u8) {
        self.decoded_data.extend(std::iter::repeat_n(byte, count));
    }

    fn end_line(&mut self) -> io::Result<()> {
        let remainder = self.decoded_data.len() % self.line_size;
        let needed = if remainder == 0 {
            self.line_size
        } else {
            self.line_size - remainder
        };
        self.copy(needed)?;
        Ok(())
    }

    pub fn decode_stream<R: Read>(&mut self, reader: &mut R) -> io::Result<()> {
        let mut byte_buf = [0u8; 1];
        let mut read_byte = |r: &mut R| -> io::Result<u8> {
            r.read_exact(&mut byte_buf)?;
            Ok(byte_buf[0])
        };

        while !self.eop_reached {
            let cmd = match read_byte(reader) {
                Ok(b) => b,
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            };

            if cmd == OP_EOP {
                self.eop_reached = true;
                break;
            } else if cmd == OP_EOL {
                self.end_line()?;
            } else if cmd == OP_NOP {
                // padding, do nothing
            } else if (cmd & 0xe0) == 0xa0 {
                // Extend
                let extend_count = ((cmd & 0x1f) as usize) << 3;
                let next_cmd = read_byte(reader)?;
                match next_cmd & 0xc0 {
                    0x00 => {
                        // RepeatXLong
                        let repeat_count = extend_count + (((next_cmd >> 3) & 7) as usize);
                        self.repeat(repeat_count, REPEAT_X_BYTE);
                    }
                    0x80 => {
                        // CopyThenRepeatLong
                        let copy_count = (next_cmd & 7) as usize;
                        let repeat_count = extend_count + (((next_cmd >> 3) & 7) as usize);
                        let repeat_byte = read_byte(reader)?;
                        self.copy(copy_count)?;
                        self.repeat(repeat_count, repeat_byte);
                    }
                    0x40 => {
                        // RepeatThenRawLong
                        let raw_count = extend_count + ((next_cmd & 7) as usize);
                        let repeat_count = ((next_cmd >> 3) & 7) as usize;
                        let repeat_byte = read_byte(reader)?;
                        let mut raw = vec![0u8; raw_count];
                        reader.read_exact(&mut raw)?;
                        self.repeat(repeat_count, repeat_byte);
                        self.decoded_data.extend_from_slice(&raw);
                    }
                    0xc0 => {
                        // CopyThenRawLong
                        let copy_count = (next_cmd & 7) as usize;
                        let raw_count = extend_count + (((next_cmd >> 3) & 7) as usize);
                        let mut raw = vec![0u8; raw_count];
                        reader.read_exact(&mut raw)?;
                        self.copy(copy_count)?;
                        self.decoded_data.extend_from_slice(&raw);
                    }
                    _ => unreachable!(),
                }
            } else if (cmd & 0xe0) == 0x80 {
                // CopyLong
                let copy_count = ((cmd & 0x1f) as usize) << 3;
                self.copy(copy_count)?;
            } else if (cmd & 0xc0) == 0x40 {
                // CopyThenRepeat
                let copy_count = (cmd & 7) as usize;
                let repeat_count = ((cmd >> 3) & 7) as usize;
                let repeat_byte = read_byte(reader)?;
                self.copy(copy_count)?;
                self.repeat(repeat_count, repeat_byte);
            } else if (cmd & 0xc0) == 0x00 {
                // CopyThenRaw
                let copy_count = (cmd & 7) as usize;
                let raw_count = ((cmd >> 3) & 7) as usize;
                let mut raw = vec![0u8; raw_count];
                reader.read_exact(&mut raw)?;
                self.copy(copy_count)?;
                self.decoded_data.extend_from_slice(&raw);
            } else if (cmd & 0xc0) == 0xc0 {
                let copy_count = (cmd & 7) as usize;
                let repeat_count = ((cmd >> 3) & 7) as usize;
                if copy_count != 0 && repeat_count == 0 {
                    // CopyShort
                    self.copy(copy_count)?;
                } else if copy_count == 0 && repeat_count != 0 {
                    // RepeatX
                    self.repeat(repeat_count, REPEAT_X_BYTE);
                } else if copy_count != 0 && repeat_count != 0 {
                    // RepeatThenRaw
                    let raw_count = copy_count;
                    let repeat_byte = read_byte(reader)?;
                    let mut raw = vec![0u8; raw_count];
                    reader.read_exact(&mut raw)?;
                    self.repeat(repeat_count, repeat_byte);
                    self.decoded_data.extend_from_slice(&raw);
                }
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Unknown SCoA command byte: 0x{:02x}", cmd),
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn roundtrip(lines: &[Vec<u8>], line_size: usize) -> Vec<u8> {
        let mut encoder = ScoaEncoder::new(line_size);
        for line in lines {
            encoder.encode_line(line);
        }
        let encoded = encoder.finish();
        assert_eq!(
            encoded.len() % 2,
            0,
            "Encoded stream must be 16-bit word aligned"
        );

        let mut decoder = ScoaDecoder::new(line_size);
        let mut cursor = Cursor::new(&encoded);
        decoder.decode_stream(&mut cursor).expect("Decoder failed");
        assert!(decoder.eop_reached, "EOP marker must be reached");

        let expected_total: usize = lines.len() * line_size;
        assert_eq!(
            decoder.decoded_data.len(),
            expected_total,
            "Decoded length mismatch"
        );

        let mut reconstructed = Vec::new();
        for line in lines {
            reconstructed.extend_from_slice(line);
        }
        assert_eq!(
            decoder.decoded_data, reconstructed,
            "Reconstructed raster does not match original"
        );

        encoded
    }

    #[test]
    fn test_finish_16bit_word_alignment() {
        for n in 0..=30 {
            let mut encoder = ScoaEncoder::new(10);
            for _ in 0..n {
                encoder.video_data.push(0x40); // OP_NOP
            }
            let res = encoder.finish();
            assert_eq!(
                res.len() % 2,
                0,
                "Failed 16-bit alignment when initial len was {}",
                n
            );
            assert_eq!(*res.last().unwrap(), OP_EOP);
        }
    }

    #[test]
    fn test_all_white_lines() {
        let line_size = 592;
        let lines = vec![vec![0u8; line_size]; 10];
        let encoded = roundtrip(&lines, line_size);
        // All white should compress significantly
        assert!(encoded.len() < lines.len() * line_size / 2);
    }

    #[test]
    fn test_all_black_lines() {
        let line_size = 592;
        let lines = vec![vec![0xFFu8; line_size]; 10];
        let encoded = roundtrip(&lines, line_size);
        assert!(encoded.len() < lines.len() * line_size / 2);
    }

    #[test]
    fn test_alternating_stripes() {
        let line_size = 592;
        let mut lines = Vec::new();
        for y in 0..20 {
            let val = if y % 2 == 0 { 0xAA } else { 0x55 };
            lines.push(vec![val; line_size]);
        }
        roundtrip(&lines, line_size);
    }

    #[test]
    fn test_repeat_x_byte() {
        // REPEAT_X_BYTE is 0x43 - tests repeat_x and repeat_x_long opcodes
        let line_size = 300;
        let lines = vec![vec![REPEAT_X_BYTE; line_size]; 5];
        roundtrip(&lines, line_size);
    }

    #[test]
    fn test_various_run_lengths() {
        for len in [1, 2, 7, 8, 9, 247, 248, 249, 255, 256, 500] {
            let mut line = vec![0x00; len];
            // Put some repeat at start
            for b in line.iter_mut().take(len.min(50)) {
                *b = 0x12;
            }
            let lines = vec![line];
            roundtrip(&lines, len);
        }
    }

    #[test]
    fn test_copy_and_eol_tail() {
        let line_size = 64;
        let line1 = vec![0x11; line_size];
        let mut line2 = line1.clone();
        // Change only first 8 bytes of line2, rest should be copied via OP_EOL
        for b in line2.iter_mut().take(8) {
            *b = 0x22;
        }
        let lines = vec![line1, line2];
        roundtrip(&lines, line_size);
    }

    #[test]
    fn test_pseudorandom_noise() {
        let line_size = 128;
        let mut lines = Vec::new();
        let mut lcg: u32 = 123456789;
        for _ in 0..25 {
            let mut line = vec![0u8; line_size];
            for byte in line.iter_mut() {
                lcg = lcg.wrapping_mul(1664525).wrapping_add(1013904223);
                *byte = (lcg >> 16) as u8;
            }
            lines.push(line);
        }
        roundtrip(&lines, line_size);
    }

    #[test]
    fn test_decoder_invalid_opcode() {
        let invalid_stream = [0xFF, 0x00];
        let mut decoder = ScoaDecoder::new(10);
        let mut cursor = Cursor::new(&invalid_stream);
        let err = decoder.decode_stream(&mut cursor);
        assert!(err.is_err());
    }

    #[test]
    fn test_decoder_truncated_stream() {
        // Stream cut off mid-command
        let truncated = [0x00]; // CopyThenRaw with 0 counts, expects raw bytes
        let mut decoder = ScoaDecoder::new(10);
        let mut cursor = Cursor::new(&truncated);
        let _ = decoder.decode_stream(&mut cursor);
        assert!(!decoder.eop_reached);
    }

    #[test]
    fn test_full_a4_roundtrip() {
        use crate::a4::generate_a4_test_page;
        let pbm = generate_a4_test_page();
        let mut lines = Vec::with_capacity(pbm.height);
        for y in 0..pbm.height {
            lines.push(pbm.line(y).to_vec());
        }
        let encoded = roundtrip(&lines, pbm.line_size);
        // Verify reasonable compression on test page
        assert!(encoded.len() < pbm.data.len());
        println!(
            "Full A4 Test Page SCoA: {} -> {} bytes ({:.2}%)",
            pbm.data.len(),
            encoded.len(),
            (encoded.len() as f64) / (pbm.data.len() as f64) * 100.0
        );
    }
}
