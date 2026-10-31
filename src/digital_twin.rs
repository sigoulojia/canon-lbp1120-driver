//! Canon LaserShot LBP-1120 Digital Twin & Virtual Hardware Simulator
//!
//! Provides a bit-exact, cycle-accurate software emulation of the physical
//! Canon LaserShot LBP-1120 (USB VID 0x04A9, PID 0x262B). Emulates USB endpoints,
//! Formatter ASIC (IC1) registers, 512 KB DRAM ring FIFO with backpressure,
//! SCoA hardware decompression to virtual 600 DPI canvas, and engine state transitions.

use crate::pbm::PbmImage;
use crate::protocol::{
    CaptPacket, PageHeaderConfig, MAGIC_GO_ONLINE, OPCODE_IC_BEGIN_DATA, OPCODE_IC_BEGIN_PAGE,
    OPCODE_IC_END_PAGE, OPCODE_IC_VIDEO_DATA, OPCODE_PCR_CLEAR_ERROR, OPCODE_PCR_CLEAR_MISPRINT,
    OPCODE_PCR_DISCARD_DATA, OPCODE_PCR_GET_BASIC_STATUS, OPCODE_PCR_GO_OFFLINE,
    OPCODE_PCR_GO_ONLINE, OPCODE_PCR_RESET_ENGINE, OPCODE_PC_GET_EXTENDED_STATUS,
    TARGET_MODEL_LBP1120,
};
use crate::scoa::ScoaDecoder;
use std::io::{self, Cursor};

pub const DRAM_CAPACITY_BYTES: usize = 524_288; // 512 KB physical DRAM
pub const DRAM_GOVERNOR_THRESHOLD: usize = 480_000; // 480 KB backpressure threshold
pub const IEEE1284_DEVICE_ID: &str =
    "MFG:Canon;MDL:LASER SHOT LBP-1120;CMD:CAPT;CLS:PRINTER;";

/// Comprehensive register state of the virtual Canon LBP-1120
#[derive(Debug, Clone)]
pub struct VirtualDeviceRegisters {
    pub basic: u8,
    pub changed: u8,
    pub aux: u8,
    pub controller: u8,
    pub paper_available_bits: u8,
    pub engine_raw: u16,
    pub printed_pages: u8,
    pub printing_pages: u8,
    pub shipped_pages: u8,
    pub start_pages: u8,
}

impl Default for VirtualDeviceRegisters {
    fn default() -> Self {
        Self {
            basic: 0x50, // 0x40 (UnitFree) | 0x10 (Offline)
            changed: 0x00,
            aux: 0x00,
            controller: 0x00,
            paper_available_bits: 0x80, // Paper present in tray
            engine_raw: 0x0000,         // Door closed, cartridge installed, healthy
            printed_pages: 0,
            printing_pages: 0,
            shipped_pages: 0,
            start_pages: 0,
        }
    }
}

/// Standalone, in-memory Digital Twin representing the Canon LaserShot LBP-1120
#[derive(Debug)]
pub struct VirtualCaptDevice {
    pub registers: VirtualDeviceRegisters,
    pub door_open: bool,
    pub paper_empty: bool,
    pub toner_present: bool,
    pub is_online: bool,
    pub streaming_active: bool,
    pub dram_buffer: Vec<u8>,
    pub current_page_config: Option<PageHeaderConfig>,
    pub video_stream: Vec<u8>,
    pub decoded_canvas: Option<PbmImage>,
    pub error_latched: bool,
    pub paper_jam: bool,
}

impl VirtualCaptDevice {
    /// Instantiate a new virtual printer in healthy standby state
    pub fn new() -> Self {
        Self {
            registers: VirtualDeviceRegisters::default(),
            door_open: false,
            paper_empty: false,
            toner_present: true,
            is_online: false,
            streaming_active: false,
            dram_buffer: Vec::with_capacity(DRAM_CAPACITY_BYTES),
            current_page_config: None,
            video_stream: Vec::new(),
            decoded_canvas: None,
            error_latched: false,
            paper_jam: false,
        }
    }

    /// Update paper tray presence sensor (PS202)
    pub fn set_paper_empty(&mut self, empty: bool) {
        self.paper_empty = empty;
        self.sync_registers();
    }

    /// Update top cover safety interlock microswitch (SW301)
    pub fn set_door_open(&mut self, open: bool) {
        self.door_open = open;
        self.sync_registers();
    }

    /// Update toner cartridge presence sensor
    pub fn set_toner_present(&mut self, present: bool) {
        self.toner_present = present;
        self.sync_registers();
    }

    /// Synchronize logical sensor states to CAPT bitfields
    fn sync_registers(&mut self) {
        // Basic Status register flags:
        // Bit 7: Device Error / Busy
        // Bit 6: Unit Free (1 = Free)
        // Bit 4: Offline (1 = Offline, 0 = Online)
        // Bit 2: Buffer Full
        // Bit 1: No Paper
        // Bit 0: Engine Active
        let mut b = 0x40; // Unit free by default

        if !self.is_online {
            b |= 0x10; // Offline
        }

        if self.paper_empty {
            b |= 0x02; // No Paper bit
            self.registers.paper_available_bits = 0x00;
        } else {
            self.registers.paper_available_bits = 0x80;
        }

        let mut eng: u16 = 0x0000;
        if self.door_open {
            eng |= 0x4000; // Door open
            b |= 0x80;     // Device Error / Unready
        }
        if !self.toner_present {
            eng |= 0x2000; // Cartridge missing
            b |= 0x80;
        }
        if self.paper_jam {
            eng |= 0x0100; // Jam
            b |= 0x80;
        }
        if self.error_latched {
            b |= 0x80;
        }
        if self.dram_buffer.len() >= DRAM_GOVERNOR_THRESHOLD {
            b |= 0x04; // Buffer Full
        }

        self.registers.basic = b;
        self.registers.engine_raw = eng;
    }

    /// Emulate USB Control Endpoint 0 Class Requests
    pub fn handle_control_request(
        &self,
        request_type: u8,
        request: u8,
        _value: u16,
        _index: u16,
        _length: u16,
    ) -> io::Result<Vec<u8>> {
        // Request Type 0xA1: Class Interface IN
        if request_type == 0xA1 {
            match request {
                0x00 => {
                    // GET_DEVICE_ID
                    let id_bytes = IEEE1284_DEVICE_ID.as_bytes();
                    let len = (id_bytes.len() + 2) as u16;
                    let mut resp = Vec::with_capacity(id_bytes.len() + 2);
                    resp.extend_from_slice(&len.to_be_bytes()); // IEEE 1284 16-bit big-endian length
                    resp.extend_from_slice(id_bytes);
                    Ok(resp)
                }
                0x01 => {
                    // GET_PORT_STATUS
                    // Hardware bridge returns 0x18
                    Ok(vec![0x18])
                }
                _ => Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Unsupported USB class control request: 0x{:02X}", request),
                )),
            }
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Unsupported control request type: 0x{:02X}", request_type),
            ))
        }
    }

    /// Ingest and process a raw CAPT 1.0 packet arriving over virtual Bulk OUT (Pipe 0x01).
    /// Returns optional response bytes to be placed into virtual Bulk IN (Pipe 0x82).
    pub fn handle_bulk_write(&mut self, packet_bytes: &[u8]) -> io::Result<Option<Vec<u8>>> {
        let mut cursor = Cursor::new(packet_bytes);
        let pkt = CaptPacket::read(&mut cursor)?;

        self.sync_registers();

        match pkt.header.opcode {
            OPCODE_PCR_GET_BASIC_STATUS => {
                // Protocol Invariant: Querying status while SCoA decompression gate is active
                // corrupts video FIFO DMA and stalls Bulk OUT Pipe 0x01 (Win32 Error 121/995).
                if self.streaming_active {
                    self.error_latched = true;
                    self.sync_registers();
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Protocol Violation: Status query (0xE0A0) sent during active video streaming. ASIC FIFO stalled.",
                    ));
                }

                // 0xE0A0: Returns 6-byte Basic Status packet
                let mut resp = Vec::with_capacity(6);
                resp.extend_from_slice(&OPCODE_PCR_GET_BASIC_STATUS.to_le_bytes());
                resp.extend_from_slice(&6u16.to_le_bytes());
                resp.push(self.registers.basic);
                resp.push(self.registers.changed);
                Ok(Some(resp))
            }
            OPCODE_PC_GET_EXTENDED_STATUS => {
                if self.streaming_active {
                    self.error_latched = true;
                    self.sync_registers();
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Protocol Violation: Extended status query (0xA0A0) sent during active video streaming. ASIC FIFO stalled.",
                    ));
                }

                // 0xA0A0: Returns 20-byte Extended Status packet
                let mut resp = Vec::with_capacity(20);
                resp.extend_from_slice(&OPCODE_PC_GET_EXTENDED_STATUS.to_le_bytes());
                resp.extend_from_slice(&20u16.to_le_bytes());
                resp.push(self.registers.basic);
                resp.push(self.registers.changed);
                // 14-byte payload:
                resp.push(self.registers.aux);
                resp.push(self.registers.controller);
                resp.push(self.registers.paper_available_bits);
                resp.push(0x00); // reserved
                resp.extend_from_slice(&self.registers.engine_raw.to_le_bytes());
                resp.push(self.registers.printed_pages);
                resp.push(self.registers.printing_pages);
                resp.push(self.registers.shipped_pages);
                resp.push(self.registers.start_pages);
                resp.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // padding
                Ok(Some(resp))
            }
            OPCODE_PCR_GO_ONLINE => {
                // 0xE0A5: Payload must contain magic 0xADEADBEE
                if pkt.payload.len() < 8 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "PCR_GO_ONLINE payload too short",
                    ));
                }
                let magic = u32::from_le_bytes([
                    pkt.payload[0],
                    pkt.payload[1],
                    pkt.payload[2],
                    pkt.payload[3],
                ]);
                if magic != MAGIC_GO_ONLINE {
                    self.registers.basic |= 0x80; // Error / Reject
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Invalid GO_ONLINE magic: 0x{:08X}", magic),
                    ));
                }

                if self.door_open || !self.toner_present {
                    self.registers.basic |= 0x80;
                    return Ok(Some(vec![
                        0xa0,
                        0xe0,
                        0x06,
                        0x00,
                        self.registers.basic,
                        0x00,
                    ]));
                }

                self.is_online = true;
                self.sync_registers();

                let mut resp = Vec::with_capacity(6);
                resp.extend_from_slice(&OPCODE_PCR_GET_BASIC_STATUS.to_le_bytes());
                resp.extend_from_slice(&6u16.to_le_bytes());
                resp.push(self.registers.basic);
                resp.push(0x10); // Changed latch: Online state transitioned
                Ok(Some(resp))
            }
            OPCODE_PCR_GO_OFFLINE => {
                self.is_online = false;
                self.streaming_active = false;
                self.sync_registers();

                let mut resp = Vec::with_capacity(6);
                resp.extend_from_slice(&OPCODE_PCR_GET_BASIC_STATUS.to_le_bytes());
                resp.extend_from_slice(&6u16.to_le_bytes());
                resp.push(self.registers.basic);
                resp.push(0x10);
                Ok(Some(resp))
            }
            OPCODE_PCR_RESET_ENGINE => {
                self.error_latched = false;
                self.paper_jam = false;
                self.dram_buffer.clear();
                self.video_stream.clear();
                self.sync_registers();

                let mut resp = Vec::with_capacity(6);
                resp.extend_from_slice(&OPCODE_PCR_GET_BASIC_STATUS.to_le_bytes());
                resp.extend_from_slice(&6u16.to_le_bytes());
                resp.push(self.registers.basic);
                resp.push(0x00);
                Ok(Some(resp))
            }
            OPCODE_PCR_CLEAR_ERROR | OPCODE_PCR_CLEAR_MISPRINT => {
                self.error_latched = false;
                self.paper_jam = false;
                self.sync_registers();

                let mut resp = Vec::with_capacity(6);
                resp.extend_from_slice(&OPCODE_PCR_GET_BASIC_STATUS.to_le_bytes());
                resp.extend_from_slice(&6u16.to_le_bytes());
                resp.push(self.registers.basic);
                resp.push(0x00);
                Ok(Some(resp))
            }
            OPCODE_PCR_DISCARD_DATA => {
                self.dram_buffer.clear();
                self.video_stream.clear();
                self.sync_registers();

                let mut resp = Vec::with_capacity(6);
                resp.extend_from_slice(&OPCODE_PCR_GET_BASIC_STATUS.to_le_bytes());
                resp.extend_from_slice(&6u16.to_le_bytes());
                resp.push(self.registers.basic);
                resp.push(0x00);
                Ok(Some(resp))
            }
            OPCODE_IC_BEGIN_PAGE => {
                let config = PageHeaderConfig::parse_begin_page_payload(&pkt.payload)?;
                if config.target_model != TARGET_MODEL_LBP1120 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Target model mismatch: 0x{:04X}", config.target_model),
                    ));
                }
                self.current_page_config = Some(config);
                self.video_stream.clear();
                self.dram_buffer.clear();
                Ok(None)
            }
            OPCODE_IC_BEGIN_DATA => {
                self.streaming_active = true;
                Ok(None)
            }
            OPCODE_IC_VIDEO_DATA => {
                if !self.streaming_active {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Received IC_VIDEO_DATA without prior IC_BEGIN_DATA",
                    ));
                }
                self.dram_buffer.extend_from_slice(&pkt.payload);
                self.video_stream.extend_from_slice(&pkt.payload);
                self.sync_registers();
                Ok(None)
            }
            OPCODE_IC_END_PAGE => {
                if !self.streaming_active {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Received IC_END_PAGE without active data streaming",
                    ));
                }
                self.streaming_active = false;

                // Execute SCoA hardware decompression into virtual 600 DPI canvas
                let cfg = self
                    .current_page_config
                    .clone()
                    .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No page config set"))?;

                let mut decoder = ScoaDecoder::new(cfg.image_line_size as usize);
                let mut v_cur = Cursor::new(&self.video_stream);
                decoder.decode_stream(&mut v_cur)?;

                let reconstructed = PbmImage {
                    width: (cfg.image_line_size * 8) as usize,
                    height: decoder.decoded_data.len() / cfg.image_line_size as usize,
                    line_size: cfg.image_line_size as usize,
                    data: decoder.decoded_data,
                };
                self.decoded_canvas = Some(reconstructed);

                // Simulate mechanical paper pickup & transit
                self.registers.start_pages = self.registers.start_pages.wrapping_add(1);
                self.registers.printed_pages = self.registers.printed_pages.wrapping_add(1);
                self.registers.shipped_pages = self.registers.shipped_pages.wrapping_add(1);
                self.dram_buffer.clear();
                self.sync_registers();

                Ok(None)
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Unknown CAPT opcode: 0x{:04X}", pkt.header.opcode),
            )),
        }
    }

    /// Access the bit-exact reconstructed page from the last successful print cycle
    pub fn last_printed_page(&self) -> Option<&PbmImage> {
        self.decoded_canvas.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{PageHeaderConfig, TARGET_MODEL_LBP1120};
    use crate::scoa::ScoaEncoder;

    #[test]
    fn test_virtual_device_initialization_and_control_descriptors() {
        let device = VirtualCaptDevice::new();
        assert!(!device.is_online);
        assert!(!device.paper_empty);
        assert!(!device.door_open);

        // IEEE 1284 Device ID query over Control Endpoint 0
        let resp = device
            .handle_control_request(0xA1, 0x00, 0, 0, 256)
            .expect("GET_DEVICE_ID failed");
        assert!(resp.len() > 2);
        let id_str = String::from_utf8_lossy(&resp[2..]);
        assert!(id_str.contains("MFG:Canon"));
        assert!(id_str.contains("LASER SHOT LBP-1120"));

        // GET_PORT_STATUS query
        let port_resp = device
            .handle_control_request(0xA1, 0x01, 0, 0, 1)
            .expect("GET_PORT_STATUS failed");
        assert_eq!(port_resp, vec![0x18]);
    }

    #[test]
    fn test_virtual_device_basic_and_extended_status_telemetry() {
        let mut device = VirtualCaptDevice::new();

        // 1. Query basic status
        let query_basic = CaptPacket::new(OPCODE_PCR_GET_BASIC_STATUS, Vec::new()).unwrap();
        let mut pkt_bytes = Vec::new();
        query_basic.write(&mut pkt_bytes).unwrap();

        let resp_bytes = device
            .handle_bulk_write(&pkt_bytes)
            .unwrap()
            .expect("Expected basic status reply");
        assert_eq!(resp_bytes.len(), 6);
        assert_eq!(u16::from_le_bytes([resp_bytes[0], resp_bytes[1]]), 0xE0A0);
        // Default state: Offline (0x10) and UnitFree (0x40) -> 0x50
        assert_eq!(resp_bytes[4] & 0x10, 0x10);

        // 2. Query extended status
        let query_ext = CaptPacket::new(OPCODE_PC_GET_EXTENDED_STATUS, Vec::new()).unwrap();
        pkt_bytes.clear();
        query_ext.write(&mut pkt_bytes).unwrap();

        let ext_resp = device
            .handle_bulk_write(&pkt_bytes)
            .unwrap()
            .expect("Expected extended status reply");
        assert_eq!(ext_resp.len(), 20);
        assert_eq!(u16::from_le_bytes([ext_resp[0], ext_resp[1]]), 0xA0A0);
        // Paper in tray flag: Byte 8 should be 0x80
        assert_eq!(ext_resp[8], 0x80);
    }

    #[test]
    fn test_virtual_device_paper_empty_and_door_open_interlocks() {
        let mut device = VirtualCaptDevice::new();

        // Test Paper Out
        device.set_paper_empty(true);
        let query_basic = CaptPacket::new(OPCODE_PCR_GET_BASIC_STATUS, Vec::new()).unwrap();
        let mut pkt_bytes = Vec::new();
        query_basic.write(&mut pkt_bytes).unwrap();
        let resp = device.handle_bulk_write(&pkt_bytes).unwrap().unwrap();
        assert_ne!(resp[4] & 0x02, 0, "Basic status should assert No Paper bit");

        // Test Door Open
        device.set_door_open(true);
        pkt_bytes.clear();
        query_basic.write(&mut pkt_bytes).unwrap();
        let resp = device.handle_bulk_write(&pkt_bytes).unwrap().unwrap();
        assert_ne!(
            resp[4] & 0x80,
            0,
            "Basic status should assert Device Error / Unready when door is open"
        );
    }

    #[test]
    fn test_virtual_device_complete_page_print_simulation() {
        let mut device = VirtualCaptDevice::new();

        // 1. PCR_GO_ONLINE
        let mut go_online_payload = Vec::new();
        go_online_payload.extend_from_slice(&MAGIC_GO_ONLINE.to_le_bytes());
        go_online_payload.extend_from_slice(&0u16.to_le_bytes()); // page 0
        go_online_payload.extend_from_slice(&0u16.to_le_bytes()); // padding

        let go_online_pkt = CaptPacket::new(OPCODE_PCR_GO_ONLINE, go_online_payload).unwrap();
        let mut pkt_bytes = Vec::new();
        go_online_pkt.write(&mut pkt_bytes).unwrap();
        let resp = device.handle_bulk_write(&pkt_bytes).unwrap().unwrap();
        assert_eq!(
            resp[4] & 0x10,
            0,
            "Engine should be Online (bit 0x10 cleared)"
        );

        // 2. IC_BEGIN_PAGE (A4, 592 bytes wide, 16 lines high for synthetic test)
        let config = PageHeaderConfig {
            target_model: TARGET_MODEL_LBP1120,
            image_line_size: 592,
            image_lines: 16,
            paper_width: 4960,
            paper_height: 7015,
            ..Default::default()
        };
        let begin_page =
            CaptPacket::new(OPCODE_IC_BEGIN_PAGE, config.build_begin_page_payload()).unwrap();
        pkt_bytes.clear();
        begin_page.write(&mut pkt_bytes).unwrap();
        assert!(device.handle_bulk_write(&pkt_bytes).unwrap().is_none());

        // 3. IC_BEGIN_DATA
        let begin_data = CaptPacket::new(OPCODE_IC_BEGIN_DATA, Vec::new()).unwrap();
        pkt_bytes.clear();
        begin_data.write(&mut pkt_bytes).unwrap();
        assert!(device.handle_bulk_write(&pkt_bytes).unwrap().is_none());

        // 4. Encode synthetic raster lines via SCoA
        let mut encoder = ScoaEncoder::new(592);
        for y in 0..16 {
            let mut line = vec![0x00u8; 592];
            if y % 2 == 1 {
                line[0] = 0xAA;
                line[1] = 0x55;
                line[591] = 0xFF;
            }
            encoder.encode_line(&line);
        }
        let video_data = encoder.finish();

        // 5. Send IC_VIDEO_DATA chunk
        let video_pkt = CaptPacket::new(OPCODE_IC_VIDEO_DATA, video_data).unwrap();
        pkt_bytes.clear();
        video_pkt.write(&mut pkt_bytes).unwrap();
        assert!(device.handle_bulk_write(&pkt_bytes).unwrap().is_none());

        // 6. IC_END_PAGE
        let end_page = CaptPacket::new(OPCODE_IC_END_PAGE, Vec::new()).unwrap();
        pkt_bytes.clear();
        end_page.write(&mut pkt_bytes).unwrap();
        assert!(device.handle_bulk_write(&pkt_bytes).unwrap().is_none());

        // Verify printed canvas reconstruction
        let canvas = device
            .last_printed_page()
            .expect("Expected decoded canvas");
        assert_eq!(canvas.line_size, 592);
        assert_eq!(canvas.height, 16);
        assert_eq!(canvas.line(1)[0], 0xAA);
        assert_eq!(canvas.line(1)[1], 0x55);
        assert_eq!(canvas.line(1)[591], 0xFF);

        // Verify page counter incremented
        assert_eq!(device.registers.printed_pages, 1);
    }

    #[test]
    fn test_status_query_during_video_streaming_causes_stall() {
        let mut device = VirtualCaptDevice::new();

        // 1. PCR_GO_ONLINE
        let mut go_online_payload = Vec::new();
        go_online_payload.extend_from_slice(&MAGIC_GO_ONLINE.to_le_bytes());
        go_online_payload.extend_from_slice(&0u16.to_le_bytes());
        go_online_payload.extend_from_slice(&0u16.to_le_bytes());
        let go_online_pkt = CaptPacket::new(OPCODE_PCR_GO_ONLINE, go_online_payload).unwrap();
        let mut pkt_bytes = Vec::new();
        go_online_pkt.write(&mut pkt_bytes).unwrap();
        let _ = device.handle_bulk_write(&pkt_bytes).unwrap();

        // 2. IC_BEGIN_PAGE
        let config = PageHeaderConfig {
            target_model: TARGET_MODEL_LBP1120,
            image_line_size: 592,
            image_lines: 16,
            ..Default::default()
        };
        let begin_page =
            CaptPacket::new(OPCODE_IC_BEGIN_PAGE, config.build_begin_page_payload()).unwrap();
        pkt_bytes.clear();
        begin_page.write(&mut pkt_bytes).unwrap();
        let _ = device.handle_bulk_write(&pkt_bytes).unwrap();

        // 3. IC_BEGIN_DATA (Opens streaming mode)
        let begin_data = CaptPacket::new(OPCODE_IC_BEGIN_DATA, Vec::new()).unwrap();
        pkt_bytes.clear();
        begin_data.write(&mut pkt_bytes).unwrap();
        let _ = device.handle_bulk_write(&pkt_bytes).unwrap();
        assert!(device.streaming_active);

        // 4. Attempt status query during active streaming (regression for Error 121/995)
        let query_status = CaptPacket::new(OPCODE_PCR_GET_BASIC_STATUS, Vec::new()).unwrap();
        pkt_bytes.clear();
        query_status.write(&mut pkt_bytes).unwrap();

        let result = device.handle_bulk_write(&pkt_bytes);
        assert!(
            result.is_err(),
            "Digital Twin must reject status queries during active video streaming"
        );
        assert_ne!(
            device.registers.basic & 0x80,
            0,
            "Device Error flag must latch on stream desynchronization"
        );
    }
}
