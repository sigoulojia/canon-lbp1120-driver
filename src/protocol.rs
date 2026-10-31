#![allow(dead_code)]
use std::io::{self, Read, Write};

pub const TARGET_MODEL_LBP1120: u16 = 0x01A4; // 420 decimal

pub const OPCODE_IC_BEGIN_PAGE: u16 = 0xD0A0;
pub const OPCODE_IC_BEGIN_DATA: u16 = 0xD0A1;
pub const OPCODE_IC_END_PAGE: u16 = 0xD0A2;
pub const OPCODE_IC_VIDEO_DATA: u16 = 0xC0A0;

pub const OPCODE_PC_GET_EXTENDED_STATUS: u16 = 0xA0A0;
pub const OPCODE_PC_GET_PRINTER_INFO: u16 = 0xA1A1;
pub const OPCODE_PCR_GET_BASIC_STATUS: u16 = 0xE0A0;
pub const OPCODE_PCR_RESET_ENGINE: u16 = 0xE0A1;
pub const OPCODE_PCR_CLEAR_ERROR: u16 = 0xE0A2;
pub const OPCODE_PCR_CLEAR_MISPRINT: u16 = 0xE0A3;
pub const OPCODE_PCR_DISCARD_DATA: u16 = 0xE0A4;
pub const OPCODE_PCR_GO_ONLINE: u16 = 0xE0A5;
pub const OPCODE_PCR_GO_OFFLINE: u16 = 0xE0A6;
pub const OPCODE_PCR_RELEASE_UNIT: u16 = 0xE0A9;
pub const OPCODE_PCR_CLEANING: u16 = 0xE0AD;

pub const MAGIC_GO_ONLINE: u32 = 0xADEADBEE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketHeader {
    pub opcode: u16,
    pub total_size: u16, // including 4-byte header
}

impl PacketHeader {
    pub fn new(opcode: u16, payload_len: usize) -> io::Result<Self> {
        let total = payload_len + 4;
        if total > u16::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Packet size exceeds 65535 bytes",
            ));
        }
        Ok(Self {
            opcode,
            total_size: total as u16,
        })
    }

    pub fn payload_len(&self) -> usize {
        (self.total_size as usize).saturating_sub(4)
    }

    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;
        let opcode = u16::from_le_bytes([buf[0], buf[1]]);
        let total_size = u16::from_le_bytes([buf[2], buf[3]]);
        if total_size < 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid packet size: {}", total_size),
            ));
        }
        Ok(Self { opcode, total_size })
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let op_bytes = self.opcode.to_le_bytes();
        let sz_bytes = self.total_size.to_le_bytes();
        writer.write_all(&[op_bytes[0], op_bytes[1], sz_bytes[0], sz_bytes[1]])?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptPacket {
    pub header: PacketHeader,
    pub payload: Vec<u8>,
}

impl CaptPacket {
    pub fn new(opcode: u16, payload: Vec<u8>) -> io::Result<Self> {
        let header = PacketHeader::new(opcode, payload.len())?;
        Ok(Self { header, payload })
    }

    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let header = PacketHeader::read(reader)?;
        let p_len = header.payload_len();
        let mut payload = vec![0u8; p_len];
        reader.read_exact(&mut payload)?;
        Ok(Self { header, payload })
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        self.header.write(writer)?;
        writer.write_all(&self.payload)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PageHeaderConfig {
    pub target_model: u16,
    pub paper_size: u8, // 2 = A4, 1 = Letter
    pub toner_density: u8,
    pub resolution: u8, // 0x11 = 600 DPI, 0 = 300 DPI
    pub smooth_enable: bool,
    pub toner_saving: bool,
    pub margin_left: u16,
    pub margin_top: u16,
    pub image_line_size: u16,
    pub image_lines: u16,
    pub paper_width: u16,
    pub paper_height: u16,
}

impl Default for PageHeaderConfig {
    fn default() -> Self {
        Self {
            target_model: TARGET_MODEL_LBP1120, // 0x01A4
            paper_size: 2,                      // A4
            toner_density: 31,
            resolution: 0x11, // 600 DPI
            smooth_enable: true,
            toner_saving: false,
            margin_left: 112,
            margin_top: 120,
            image_line_size: 592,
            image_lines: 6778,
            paper_width: 4960,  // 210 mm @ 600 dpi
            paper_height: 7015, // 297 mm @ 600 dpi
        }
    }
}

impl PageHeaderConfig {
    pub fn build_begin_page_payload(&self) -> Vec<u8> {
        let mut p = Vec::with_capacity(34);
        p.extend_from_slice(&0u16.to_le_bytes()); // 0..1: const 0
        p.extend_from_slice(&self.target_model.to_le_bytes()); // 2..3: TargetModel (0x01a4)
        p.push(self.paper_size); // 4: PaperSize
        p.push(1); // 5: const 1
        p.push(0); // 6: InputSlot
        p.push(0); // 7: const 0
        p.push(self.toner_density); // 8: TonerDensity
        p.push(self.toner_density); // 9: TonerDensity
        p.push(self.toner_density); // 10: TonerDensity
        p.push(self.toner_density); // 11: TonerDensity
        p.push(0); // 12: Mode
        p.push(self.resolution); // 13: Resolution (0x11)
        p.push(3); // 14: const 3
        p.push(1); // 15: const 1
        p.push(1); // 16: const 1
        p.push(1); // 17: const 1
        p.push(if self.smooth_enable { 2 } else { 0 }); // 18: SmoothEnable
        p.push(if self.toner_saving { 1 } else { 0 }); // 19: TonerSaving
        p.push(0); // 20: const 0
        p.push(0); // 21: const 0
        p.extend_from_slice(&self.margin_left.to_le_bytes()); // 22..23: MarginLeft
        p.extend_from_slice(&self.margin_top.to_le_bytes()); // 24..25: MarginTop
        p.extend_from_slice(&self.image_line_size.to_le_bytes()); // 26..27: ImageLineSize
        p.extend_from_slice(&self.image_lines.to_le_bytes()); // 28..29: ImageLines
        p.extend_from_slice(&self.paper_width.to_le_bytes()); // 30..31: PaperWidth
        p.extend_from_slice(&self.paper_height.to_le_bytes()); // 32..33: PaperHeight
        assert_eq!(p.len(), 34);
        p
    }

    pub fn parse_begin_page_payload(payload: &[u8]) -> io::Result<Self> {
        if payload.len() < 34 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("IC_BEGIN_PAGE payload too short: {} bytes", payload.len()),
            ));
        }
        let target_model = u16::from_le_bytes([payload[2], payload[3]]);
        let paper_size = payload[4];
        let toner_density = payload[8];
        let resolution = payload[13];
        let smooth_enable = payload[18] != 0;
        let toner_saving = payload[19] != 0;
        let margin_left = u16::from_le_bytes([payload[22], payload[23]]);
        let margin_top = u16::from_le_bytes([payload[24], payload[25]]);
        let image_line_size = u16::from_le_bytes([payload[26], payload[27]]);
        let image_lines = u16::from_le_bytes([payload[28], payload[29]]);
        let paper_width = u16::from_le_bytes([payload[30], payload[31]]);
        let paper_height = u16::from_le_bytes([payload[32], payload[33]]);

        Ok(Self {
            target_model,
            paper_size,
            toner_density,
            resolution,
            smooth_enable,
            toner_saving,
            margin_left,
            margin_top,
            image_line_size,
            image_lines,
            paper_width,
            paper_height,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_packet_header_roundtrip() {
        let header = PacketHeader::new(OPCODE_IC_BEGIN_PAGE, 34).unwrap();
        assert_eq!(header.opcode, OPCODE_IC_BEGIN_PAGE);
        assert_eq!(header.total_size, 38);
        assert_eq!(header.payload_len(), 34);

        let mut buf = Vec::new();
        header.write(&mut buf).unwrap();
        assert_eq!(buf.len(), 4);

        let mut cursor = Cursor::new(&buf);
        let recovered = PacketHeader::read(&mut cursor).unwrap();
        assert_eq!(header, recovered);
    }

    #[test]
    fn test_packet_size_overflow() {
        let err = PacketHeader::new(0x1234, 65535);
        assert!(err.is_err(), "Payload len 65535 + 4 exceeds u16::MAX");
    }

    #[test]
    fn test_packet_header_invalid_size() {
        let bad_bytes = [0x01, 0x00, 0x03, 0x00]; // size = 3, less than minimum 4
        let mut cursor = Cursor::new(&bad_bytes);
        assert!(PacketHeader::read(&mut cursor).is_err());
    }

    #[test]
    fn test_capt_packet_roundtrip() {
        let payload = vec![0x10, 0x20, 0x30, 0x40, 0x50];
        let pkt = CaptPacket::new(OPCODE_IC_VIDEO_DATA, payload.clone()).unwrap();

        let mut stream = Vec::new();
        pkt.write(&mut stream).unwrap();
        assert_eq!(stream.len(), 4 + payload.len());

        let mut cursor = Cursor::new(&stream);
        let recovered = CaptPacket::read(&mut cursor).unwrap();
        assert_eq!(pkt, recovered);
        assert_eq!(recovered.payload, payload);
    }

    #[test]
    fn test_capt_packet_truncated_payload() {
        let pkt = CaptPacket::new(OPCODE_IC_VIDEO_DATA, vec![0xAA; 20]).unwrap();
        let mut stream = Vec::new();
        pkt.write(&mut stream).unwrap();

        // Truncate stream so payload is incomplete
        stream.truncate(stream.len() - 5);
        let mut cursor = Cursor::new(&stream);
        assert!(CaptPacket::read(&mut cursor).is_err());
    }

    #[test]
    fn test_page_header_config_roundtrip() {
        let config = PageHeaderConfig::default();
        let payload = config.build_begin_page_payload();
        assert_eq!(payload.len(), 34);

        let parsed = PageHeaderConfig::parse_begin_page_payload(&payload).unwrap();
        assert_eq!(parsed.target_model, TARGET_MODEL_LBP1120);
        assert_eq!(parsed.paper_size, 2);
        assert_eq!(parsed.resolution, 0x11);
        assert_eq!(parsed.image_line_size, 592);
        assert_eq!(parsed.image_lines, 6778);
        assert_eq!(parsed.paper_width, 4960);
        assert_eq!(parsed.paper_height, 7015);
        assert_eq!(parsed.margin_left, 112);
        assert_eq!(parsed.margin_top, 120);
    }

    #[test]
    fn test_page_header_config_too_short() {
        let short_payload = vec![0u8; 33];
        assert!(PageHeaderConfig::parse_begin_page_payload(&short_payload).is_err());
    }
}
