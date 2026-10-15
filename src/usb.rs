#![allow(dead_code, clippy::upper_case_acronyms)]
use std::ffi::c_void;
use std::io;
use std::ptr;

type HANDLE = *mut c_void;
type BOOL = i32;
type PVOID = *mut c_void;

const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;
const GENERIC_READ: u32 = 0x80000000;
const GENERIC_WRITE: u32 = 0x40000000;
const FILE_SHARE_READ: u32 = 1;
const FILE_SHARE_WRITE: u32 = 2;
const OPEN_EXISTING: u32 = 3;
const FILE_FLAG_OVERLAPPED: u32 = 0x40000000;

// WinUSB Pipe Policies
const SHORT_PACKET_TERMINATE: u32 = 1;
const AUTO_CLEAR_STALL: u32 = 2;
const PIPE_TRANSFER_TIMEOUT: u32 = 3;
const ALLOW_PARTIAL_READS: u32 = 5;
const RAW_IO: u32 = 7;

#[repr(C, packed)]
struct WinUsbSetupPacket {
    request_type: u8,
    request: u8,
    value: u16,
    index: u16,
    length: u16,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateFileW(
        lpFileName: *const u16,
        dwDesiredAccess: u32,
        dwShareMode: u32,
        lpSecurityAttributes: *mut c_void,
        dwCreationDisposition: u32,
        dwFlagsAndAttributes: u32,
        hTemplateFile: HANDLE,
    ) -> HANDLE;
    fn CloseHandle(hObject: HANDLE) -> BOOL;
    fn GetLastError() -> u32;
}

#[link(name = "winusb")]
unsafe extern "system" {
    fn WinUsb_Initialize(DeviceHandle: HANDLE, InterfaceHandle: *mut PVOID) -> BOOL;
    fn WinUsb_Free(InterfaceHandle: PVOID) -> BOOL;
    fn WinUsb_ControlTransfer(
        InterfaceHandle: PVOID,
        SetupPacket: WinUsbSetupPacket,
        Buffer: *mut u8,
        BufferLength: u32,
        LengthTransferred: *mut u32,
        Overlapped: *mut c_void,
    ) -> BOOL;
    fn WinUsb_ReadPipe(
        InterfaceHandle: PVOID,
        PipeID: u8,
        Buffer: *mut u8,
        BufferLength: u32,
        LengthTransferred: *mut u32,
        Overlapped: *mut c_void,
    ) -> BOOL;
    fn WinUsb_WritePipe(
        InterfaceHandle: PVOID,
        PipeID: u8,
        Buffer: *const u8,
        BufferLength: u32,
        LengthTransferred: *mut u32,
        Overlapped: *mut c_void,
    ) -> BOOL;
    fn WinUsb_SetPipePolicy(
        InterfaceHandle: PVOID,
        PipeID: u8,
        PolicyType: u32,
        ValueLength: u32,
        Value: *const c_void,
    ) -> BOOL;
    fn WinUsb_ResetPipe(InterfaceHandle: PVOID, PipeID: u8) -> BOOL;
    fn WinUsb_AbortPipe(InterfaceHandle: PVOID, PipeID: u8) -> BOOL;
    fn WinUsb_FlushPipe(InterfaceHandle: PVOID, PipeID: u8) -> BOOL;
    fn WinUsb_QueryInterfaceSettings(
        InterfaceHandle: PVOID,
        AlternateInterfaceNumber: u8,
        UsbAltInterfaceDescriptor: *mut UsbInterfaceDescriptor,
    ) -> BOOL;
    fn WinUsb_QueryPipe(
        InterfaceHandle: PVOID,
        AlternateInterfaceNumber: u8,
        PipeIndex: u8,
        PipeInformation: *mut WinUsbPipeInformation,
    ) -> BOOL;
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct UsbInterfaceDescriptor {
    pub length: u8,
    pub descriptor_type: u8,
    pub interface_number: u8,
    pub alternate_setting: u8,
    pub num_endpoints: u8,
    pub interface_class: u8,
    pub interface_sub_class: u8,
    pub interface_protocol: u8,
    pub interface_index: u8,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct WinUsbPipeInformation {
    pub pipe_type: u32,
    pub pipe_id: u8,
    pub maximum_packet_size: u16,
    pub interval: u8,
}

#[derive(Debug, Clone)]
pub struct UsbPortStatus {
    pub raw: u8,
    pub paper_empty: bool,
    pub selected: bool,
    pub no_error: bool,
}

#[derive(Debug, Clone)]
pub struct ExtendedStatusReport {
    pub basic: u8,
    pub aux: u8,
    pub controller: u8,
    pub paper_available_bits: u8,
    pub engine_raw: u16,
    pub door_open: bool,
    pub no_cartridge: bool,
    pub paper_jam: bool,
    pub no_paper: bool,
    pub waiting: bool,
    pub cleaning: bool,
    pub service_call: bool,
    pub unit_free: bool,
    pub online: bool,
    pub start_pages: u16,
    pub printing_pages: u16,
    pub shipped_pages: u16,
    pub printed_pages: u16,
}

pub struct UsbPrinter {
    file_handle: HANDLE,
    winusb_handle: PVOID,
    pub path: String,
}

impl UsbPrinter {
    pub fn find_and_open() -> io::Result<Self> {
        let candidates = [
            r"\\?\USB#VID_04A9&PID_262B#0353S4cQ#{F9A25620-E255-4B37-93F3-0C3D3D2DFBD3}",
            r"\\?\USB#VID_04A9&PID_262B#0353S4cQ#{a5dcbf10-6530-11d2-901f-00c04fb951ed}",
        ];

        for path in &candidates {
            if let Ok(dev) = Self::open(path) {
                return Ok(dev);
            }
        }

        // Try querying registry if explicit candidate paths fail
        if let Ok(output) = std::process::Command::new("reg")
            .args([
                "query",
                r"HKLM\SYSTEM\CurrentControlSet\Enum\USB\VID_04A9&PID_262B",
                "/s",
            ])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                if line.contains("SymbolicName")
                    && let Some(val) = line.split("REG_SZ").nth(1)
                {
                    let trimmed = val.trim();
                    let clean_path = trimmed.replace(r"\??\", r"\\?\");
                    if let Ok(dev) = Self::open(&clean_path) {
                        return Ok(dev);
                    }
                }
            }
        }

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Canon LaserShot LBP-1120 (04A9:262B) not found or could not be opened with WinUSB",
        ))
    }

    pub fn open(device_path: &str) -> io::Result<Self> {
        let wide: Vec<u16> = device_path
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let h_file = unsafe {
            CreateFileW(
                wide.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null_mut(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                ptr::null_mut(),
            )
        };

        if h_file == INVALID_HANDLE_VALUE || h_file.is_null() {
            let err = unsafe { GetLastError() };
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "CreateFileW failed for '{}' (Win32 error: {})",
                    device_path, err
                ),
            ));
        }

        let mut winusb: PVOID = ptr::null_mut();
        let res = unsafe { WinUsb_Initialize(h_file, &mut winusb) };
        if res == 0 {
            let err = unsafe { GetLastError() };
            unsafe { CloseHandle(h_file) };
            return Err(io::Error::other(format!(
                "WinUsb_Initialize failed (Win32 error: {})",
                err
            )));
        }

        // Configure default pipe policies
        unsafe {
            let timeout: u32 = 5000;
            WinUsb_SetPipePolicy(
                winusb,
                0x01,
                PIPE_TRANSFER_TIMEOUT,
                4,
                &timeout as *const _ as *const c_void,
            );
            WinUsb_SetPipePolicy(
                winusb,
                0x82,
                PIPE_TRANSFER_TIMEOUT,
                4,
                &timeout as *const _ as *const c_void,
            );

            let allow_partial: u8 = 1;
            WinUsb_SetPipePolicy(
                winusb,
                0x82,
                ALLOW_PARTIAL_READS,
                1,
                &allow_partial as *const _ as *const c_void,
            );

            let auto_stall: u8 = 1;
            WinUsb_SetPipePolicy(
                winusb,
                0x01,
                AUTO_CLEAR_STALL,
                1,
                &auto_stall as *const _ as *const c_void,
            );
            WinUsb_SetPipePolicy(
                winusb,
                0x82,
                AUTO_CLEAR_STALL,
                1,
                &auto_stall as *const _ as *const c_void,
            );
        }

        Ok(Self {
            file_handle: h_file,
            winusb_handle: winusb,
            path: device_path.to_string(),
        })
    }

    pub fn get_device_id(&self) -> io::Result<String> {
        let pkt = WinUsbSetupPacket {
            request_type: 0xA1,
            request: 0,
            value: 0,
            index: 0,
            length: 256,
        };
        let mut buf = [0u8; 256];
        let mut trans = 0u32;
        let res = unsafe {
            WinUsb_ControlTransfer(
                self.winusb_handle,
                pkt,
                buf.as_mut_ptr(),
                buf.len() as u32,
                &mut trans,
                ptr::null_mut(),
            )
        };
        if res == 0 || trans < 2 {
            let err = unsafe { GetLastError() };
            return Err(io::Error::other(format!(
                "GET_DEVICE_ID failed (Win32 error: {})",
                err
            )));
        }
        let length = ((buf[0] as usize) << 8) | (buf[1] as usize);
        let actual_len = length.min((trans as usize).saturating_sub(2));
        Ok(String::from_utf8_lossy(&buf[2..2 + actual_len]).to_string())
    }

    pub fn get_port_status(&self) -> io::Result<UsbPortStatus> {
        let pkt = WinUsbSetupPacket {
            request_type: 0xA1,
            request: 1,
            value: 0,
            index: 0,
            length: 1,
        };
        let mut b = 0u8;
        let mut trans = 0u32;
        let res = unsafe {
            WinUsb_ControlTransfer(
                self.winusb_handle,
                pkt,
                &mut b,
                1,
                &mut trans,
                ptr::null_mut(),
            )
        };
        if res == 0 {
            let err = unsafe { GetLastError() };
            return Err(io::Error::other(format!(
                "GET_PORT_STATUS failed (Win32 error: {})",
                err
            )));
        }
        Ok(Self::parse_port_status(b))
    }

    pub fn parse_port_status(b: u8) -> UsbPortStatus {
        UsbPortStatus {
            raw: b,
            paper_empty: (b & 0x20) != 0,
            selected: (b & 0x10) != 0,
            no_error: (b & 0x08) != 0,
        }
    }

    pub fn write_pipe(&self, pipe_id: u8, data: &[u8]) -> io::Result<usize> {
        let mut total_written = 0;
        while total_written < data.len() {
            let mut transferred = 0u32;
            let chunk = &data[total_written..];
            let res = unsafe {
                WinUsb_WritePipe(
                    self.winusb_handle,
                    pipe_id,
                    chunk.as_ptr(),
                    chunk.len() as u32,
                    &mut transferred,
                    ptr::null_mut(),
                )
            };
            if res == 0 {
                let err = unsafe { GetLastError() };
                eprintln!(
                    "[USB Write FAIL] Pipe 0x{:02X}: requested {} bytes, written {}/{} bytes, Win32 error: {}",
                    pipe_id,
                    chunk.len(),
                    total_written,
                    data.len(),
                    err
                );
                return Err(io::Error::other(format!(
                    "WinUsb_WritePipe failed (Pipe 0x{:02X}, Win32 error: {})",
                    pipe_id, err
                )));
            }
            if transferred == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    format!(
                        "WinUsb_WritePipe transferred 0 bytes on Pipe 0x{:02X}",
                        pipe_id
                    ),
                ));
            }
            total_written += transferred as usize;
        }
        Ok(total_written)
    }

    pub fn read_pipe(&self, pipe_id: u8, buf: &mut [u8]) -> io::Result<usize> {
        let mut transferred = 0u32;
        let res = unsafe {
            WinUsb_ReadPipe(
                self.winusb_handle,
                pipe_id,
                buf.as_mut_ptr(),
                buf.len() as u32,
                &mut transferred,
                ptr::null_mut(),
            )
        };
        if res == 0 {
            let err = unsafe { GetLastError() };
            return Err(io::Error::other(format!(
                "WinUsb_ReadPipe failed (Pipe 0x{:02X}, Win32 error: {})",
                pipe_id, err
            )));
        }
        Ok(transferred as usize)
    }

    pub fn read_pipe_exact(&self, pipe_id: u8, buf: &mut [u8]) -> io::Result<()> {
        let mut total_read = 0;
        while total_read < buf.len() {
            let n = self.read_pipe(pipe_id, &mut buf[total_read..])?;
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    format!(
                        "Unexpected EOF on Pipe 0x{:02X} (read {}/{} bytes)",
                        pipe_id,
                        total_read,
                        buf.len()
                    ),
                ));
            }
            total_read += n;
        }
        Ok(())
    }

    pub fn reset_pipes(&self) -> io::Result<()> {
        unsafe {
            WinUsb_ResetPipe(self.winusb_handle, 0x01);
            WinUsb_ResetPipe(self.winusb_handle, 0x82);
        }
        Ok(())
    }

    pub fn parse_extended_status(
        header: &[u8; 6],
        payload: &[u8],
    ) -> io::Result<ExtendedStatusReport> {
        let opcode = u16::from_le_bytes([header[0], header[1]]);
        if opcode != 0xA0A0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Unexpected opcode 0x{:04X} in extended status reply",
                    opcode
                ),
            ));
        }

        let basic = header[4];
        let aux = payload.first().copied().unwrap_or(0);
        let controller = payload.get(1).copied().unwrap_or(0);
        let paper_available_bits = payload.get(2).copied().unwrap_or(0);
        let engine_raw = if payload.len() >= 6 {
            u16::from_le_bytes([payload[4], payload[5]])
        } else {
            0
        };

        let start_pages = if payload.len() >= 8 {
            u16::from_le_bytes([payload[6], payload[7]])
        } else {
            0
        };
        let printing_pages = if payload.len() >= 10 {
            u16::from_le_bytes([payload[8], payload[9]])
        } else {
            0
        };
        let shipped_pages = if payload.len() >= 12 {
            u16::from_le_bytes([payload[10], payload[11]])
        } else {
            0
        };
        let printed_pages = if payload.len() >= 14 {
            u16::from_le_bytes([payload[12], payload[13]])
        } else {
            0
        };

        Ok(ExtendedStatusReport {
            basic,
            aux,
            controller,
            paper_available_bits,
            engine_raw,
            door_open: (engine_raw & 0x4000) != 0,
            no_cartridge: (engine_raw & 0x2000) != 0,
            waiting: (engine_raw & 0x1000) != 0,
            no_paper: (engine_raw & 0x0200) != 0 || (paper_available_bits & 0x80) == 0,
            paper_jam: (engine_raw & 0x0100) != 0,
            cleaning: (engine_raw & 0x0004) != 0,
            service_call: (engine_raw & 0x0002) != 0,
            unit_free: (basic & 0x40) != 0,
            online: (basic & 0x10) == 0,
            start_pages,
            printing_pages,
            shipped_pages,
            printed_pages,
        })
    }

    pub fn query_extended_status(&self) -> io::Result<ExtendedStatusReport> {
        // Send PC_GET_EXTENDED_STATUS (0xA0A0, size 4)
        let cmd = [0xa0, 0xa0, 0x04, 0x00];
        self.write_pipe(0x01, &cmd)?;

        // First read: 6-byte header
        let mut header = [0u8; 6];
        self.read_pipe_exact(0x82, &mut header)?;

        let total_size = u16::from_le_bytes([header[2], header[3]]);
        let payload_len = (total_size as usize).saturating_sub(6);
        let mut payload = vec![0u8; payload_len];
        if !payload.is_empty() {
            self.read_pipe_exact(0x82, &mut payload)?;
        }

        Self::parse_extended_status(&header, &payload)
    }

    pub fn query_basic_status(&self) -> io::Result<[u8; 6]> {
        // Send PCR_GET_BASIC_STATUS (0xE0A0, size 4)
        let cmd = [0xa0, 0xe0, 0x04, 0x00];
        self.write_pipe(0x01, &cmd)?;
        let mut buf = [0u8; 6];
        self.read_pipe_exact(0x82, &mut buf)?;
        Ok(buf)
    }

    pub fn exec_cmd(&self, opcode: u16) -> io::Result<u8> {
        let cmd = [opcode as u8, (opcode >> 8) as u8, 0x04, 0x00];
        self.write_pipe(0x01, &cmd)?;
        let mut resp = [0u8; 6];
        let n = self.read_pipe(0x82, &mut resp)?;
        let err_code = if n >= 5 { resp[4] } else { 0 };
        Ok(err_code)
    }

    pub fn clear_error(&self) -> io::Result<()> {
        let _ = self.exec_cmd(0xE0A2); // PCR_CLEAR_ERROR
        Ok(())
    }

    pub fn discard_data(&self) -> io::Result<()> {
        let _ = self.exec_cmd(0xE0A4); // PCR_DISCARD_DATA
        Ok(())
    }

    pub fn reserve_unit(&self) -> io::Result<()> {
        let _ = self.exec_cmd(0xA2A0); // PC_RESERVE_UNIT
        Ok(())
    }

    pub fn go_online(&self, page_number: u16) -> io::Result<()> {
        // PCR_GO_ONLINE: 0xE0A5, size 12
        let mut cmd = Vec::with_capacity(12);
        cmd.extend_from_slice(&0xE0A5u16.to_le_bytes()); // Opcode
        cmd.extend_from_slice(&12u16.to_le_bytes()); // Size = 12
        cmd.extend_from_slice(&0xADEADBEEu32.to_le_bytes()); // Magic 0xADEADBEE
        cmd.extend_from_slice(&page_number.to_le_bytes());
        cmd.extend_from_slice(&[0x00, 0x00]);
        self.write_pipe(0x01, &cmd)?;
        let mut buf = [0u8; 6];
        let _ = self.read_pipe(0x82, &mut buf);
        Ok(())
    }

    pub fn go_offline(&self) -> io::Result<()> {
        let _ = self.exec_cmd(0xE0A6); // PCR_GO_OFFLINE
        Ok(())
    }

    pub fn release_unit(&self) -> io::Result<()> {
        let _ = self.exec_cmd(0xE0A9); // PCR_RELEASE_UNIT
        Ok(())
    }

    pub fn reset_engine(&self) -> io::Result<()> {
        let _ = self.exec_cmd(0xE0A1); // PCR_RESET_ENGINE
        Ok(())
    }

    pub fn clear_misprint(&self) -> io::Result<()> {
        let _ = self.exec_cmd(0xE0A3); // PCR_CLEAR_MISPRINT
        Ok(())
    }

    pub fn full_recovery(&self) {
        let _ = self.reset_pipes();
        let _ = self.clear_error();
        let _ = self.discard_data();
        let _ = self.go_offline();
        let _ = self.release_unit();
        let _ = self.reset_pipes();
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    pub fn query_pipes(&self) -> io::Result<Vec<WinUsbPipeInformation>> {
        let mut desc = UsbInterfaceDescriptor::default();
        let res = unsafe { WinUsb_QueryInterfaceSettings(self.winusb_handle, 0, &mut desc) };
        if res == 0 {
            let err = unsafe { GetLastError() };
            return Err(io::Error::other(format!(
                "WinUsb_QueryInterfaceSettings failed: {}",
                err
            )));
        }
        let mut pipes = Vec::new();
        for i in 0..desc.num_endpoints {
            let mut info = WinUsbPipeInformation::default();
            let p_res = unsafe { WinUsb_QueryPipe(self.winusb_handle, 0, i, &mut info) };
            if p_res != 0 {
                pipes.push(info);
            }
        }
        Ok(pipes)
    }
}

impl Drop for UsbPrinter {
    fn drop(&mut self) {
        if !self.winusb_handle.is_null() {
            unsafe { WinUsb_Free(self.winusb_handle) };
            self.winusb_handle = ptr::null_mut();
        }
        if !self.file_handle.is_null() && self.file_handle != INVALID_HANDLE_VALUE {
            unsafe { CloseHandle(self.file_handle) };
            self.file_handle = INVALID_HANDLE_VALUE;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_status_decoding() {
        // Selected (0x10), No Error (0x08), Paper Not Empty (0x00)
        let st = UsbPrinter::parse_port_status(0x18);
        assert!(st.selected);
        assert!(st.no_error);
        assert!(!st.paper_empty);

        // Paper Empty (0x20)
        let st2 = UsbPrinter::parse_port_status(0x28);
        assert!(st2.paper_empty);
        assert!(st2.no_error);
        assert!(!st2.selected);
    }

    #[test]
    fn test_extended_status_healthy_decoding() {
        // Header: Opcode A0A0, size 20 (0x0014), basic 0x40 (UnitFree=true, Online=true)
        let header = [0xA0, 0xA0, 0x14, 0x00, 0x40, 0x00];
        // Payload: aux=0x00, controller=0x00, paper_avail=0x80 (paper in tray),
        // pad=0, engine_raw=0x0000 (all healthy),
        // start_pages=1, printing_pages=1, shipped_pages=1, printed_pages=1
        let payload = [
            0x00, 0x00, 0x80, 0x00, 0x00, 0x00, // engine_raw
            0x01, 0x00, // start_pages = 1
            0x01, 0x00, // printing_pages = 1
            0x01, 0x00, // shipped_pages = 1
            0x01, 0x00, // printed_pages = 1
        ];

        let report = UsbPrinter::parse_extended_status(&header, &payload).unwrap();
        assert!(report.unit_free);
        assert!(report.online);
        assert!(!report.door_open);
        assert!(!report.no_cartridge);
        assert!(!report.paper_jam);
        assert!(!report.no_paper);
        assert!(!report.waiting);
        assert!(!report.service_call);
        assert_eq!(report.shipped_pages, 1);
    }

    #[test]
    fn test_extended_status_error_flags() {
        let header = [0xA0, 0xA0, 0x14, 0x00, 0x00, 0x00];
        // engine_raw = 0x4100 (door_open=0x4000 | paper_jam=0x0100)
        let payload = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x41, // engine_raw = 0x4100
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let report = UsbPrinter::parse_extended_status(&header, &payload).unwrap();
        assert!(report.door_open);
        assert!(report.paper_jam);
        assert!(report.no_paper); // paper_available_bits & 0x80 == 0
    }

    #[test]
    fn test_extended_status_invalid_opcode() {
        let bad_header = [0x12, 0x34, 0x06, 0x00, 0x00, 0x00];
        let payload = [];
        assert!(UsbPrinter::parse_extended_status(&bad_header, &payload).is_err());
    }
}
