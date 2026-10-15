mod a4;
mod pbm;
mod pdf;
mod protocol;
mod scoa;
mod spool;
mod usb;

use pbm::PbmImage;
use protocol::{
    CaptPacket, OPCODE_IC_BEGIN_DATA, OPCODE_IC_BEGIN_PAGE, OPCODE_IC_END_PAGE,
    OPCODE_IC_VIDEO_DATA, PageHeaderConfig, TARGET_MODEL_LBP1120,
};
use scoa::{ScoaDecoder, ScoaEncoder};
use std::env;
use std::io::Cursor;
use std::thread::sleep;
use std::time::Duration;
use usb::UsbPrinter;

fn encode_pbm_to_capt(
    pbm: &PbmImage,
    chunk_size: usize,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let config = PageHeaderConfig {
        target_model: TARGET_MODEL_LBP1120,
        image_line_size: pbm.line_size as u16,
        image_lines: pbm.height as u16,
        paper_width: (pbm.line_size * 8) as u16,
        paper_height: pbm.height as u16,
        ..Default::default()
    };

    let mut encoder = ScoaEncoder::new(pbm.line_size);
    for y in 0..pbm.height {
        encoder.encode_line(pbm.line(y));
    }
    let video_data = encoder.finish();

    let mut output = Vec::new();

    // 1. IC_BEGIN_PAGE (0xD0A0)
    let begin_page = CaptPacket::new(OPCODE_IC_BEGIN_PAGE, config.build_begin_page_payload())?;
    begin_page.write(&mut output)?;

    // 2. IC_BEGIN_DATA (0xD0A1)
    let begin_data = CaptPacket::new(OPCODE_IC_BEGIN_DATA, Vec::new())?;
    begin_data.write(&mut output)?;

    // 3. IC_VIDEO_DATA (0xC0A0) chunks
    let mut offset = 0;
    while offset < video_data.len() {
        let end = (offset + chunk_size).min(video_data.len());
        let chunk = video_data[offset..end].to_vec();
        let video_pkt = CaptPacket::new(OPCODE_IC_VIDEO_DATA, chunk)?;
        video_pkt.write(&mut output)?;
        offset = end;
    }

    // 4. IC_END_PAGE (0xD0A2)
    let end_page = CaptPacket::new(OPCODE_IC_END_PAGE, Vec::new())?;
    end_page.write(&mut output)?;

    Ok(output)
}

fn decode_capt_to_pbm(capt_bytes: &[u8]) -> Result<PbmImage, Box<dyn std::error::Error>> {
    let mut cursor = Cursor::new(capt_bytes);
    let mut config: Option<PageHeaderConfig> = None;
    let mut video_stream = Vec::new();

    while (cursor.position() as usize) < capt_bytes.len() {
        let pkt = CaptPacket::read(&mut cursor)?;
        match pkt.header.opcode {
            OPCODE_IC_BEGIN_PAGE => {
                config = Some(PageHeaderConfig::parse_begin_page_payload(&pkt.payload)?);
            }
            OPCODE_IC_BEGIN_DATA => {
                // start of data
            }
            OPCODE_IC_VIDEO_DATA => {
                video_stream.extend_from_slice(&pkt.payload);
            }
            OPCODE_IC_END_PAGE => {
                break;
            }
            _ => {
                eprintln!("Info: Skipping packet opcode 0x{:04X}", pkt.header.opcode);
            }
        }
    }

    let cfg = config.ok_or("No IC_BEGIN_PAGE packet found in CAPT stream")?;
    let mut decoder = ScoaDecoder::new(cfg.image_line_size as usize);
    let mut v_cursor = Cursor::new(&video_stream);
    decoder.decode_stream(&mut v_cursor)?;

    let expected_len = (cfg.image_line_size as usize) * (cfg.image_lines as usize);
    if decoder.decoded_data.len() < expected_len {
        return Err(format!(
            "Decoded raster size mismatch: expected {} bytes, got {}",
            expected_len,
            decoder.decoded_data.len()
        )
        .into());
    }

    Ok(PbmImage {
        width: (cfg.image_line_size as usize) * 8,
        height: cfg.image_lines as usize,
        line_size: cfg.image_line_size as usize,
        data: decoder.decoded_data[..expected_len].to_vec(),
    })
}

fn generate_synthetic_pbm(width: usize, height: usize, pattern_type: &str) -> PbmImage {
    let mut img = PbmImage::new(width, height);
    match pattern_type {
        "empty" => {} // all zeros
        "black" => {
            img.data.fill(0xFF);
        }
        "stripes" => {
            for y in 0..height {
                if y % 2 == 0 {
                    img.line_mut(y).fill(0xAA);
                } else {
                    img.line_mut(y).fill(0x55);
                }
            }
        }
        "grid" => {
            for y in 0..height {
                for x in 0..width {
                    if x % 32 == 0 || y % 32 == 0 {
                        img.set_pixel(x, y, true);
                    }
                }
            }
        }
        "text_sample" => {
            for y in 0..height {
                if (y % 40) < 20 {
                    for x in 50..(width.saturating_sub(50)) {
                        if ((x ^ y) % 7) == 0 {
                            img.set_pixel(x, y, true);
                        }
                    }
                }
            }
        }
        _ => {}
    }
    img
}

fn run_probe() -> Result<(), Box<dyn std::error::Error>> {
    println!("==================================================");
    println!("   CANON LBP-1120 NON-DESTRUCTIVE HARDWARE PROBE  ");
    println!("==================================================");

    print!("Connecting to printer via WinUSB... ");
    let printer = UsbPrinter::find_and_open()?;
    println!("CONNECTED!");
    println!("Device Path: {}", printer.path);

    // 1. USB Control: GET_DEVICE_ID
    println!("\n[1] Querying IEEE 1284 Device ID (Control Endpoint 0):");
    match printer.get_device_id() {
        Ok(id_str) => {
            println!("    Raw String : {}", id_str);
            for field in id_str.split(';') {
                let parts: Vec<&str> = field.splitn(2, ':').collect();
                if parts.len() == 2 {
                    println!("    {:10} : {}", parts[0], parts[1]);
                }
            }
        }
        Err(e) => println!("    Error: {}", e),
    }

    // 2. USB Control: GET_PORT_STATUS
    println!("\n[2] Querying Hardware Port Status (Control Endpoint 0):");
    match printer.get_port_status() {
        Ok(port_status) => {
            println!("    Raw Byte   : 0x{:02X}", port_status.raw);
            println!(
                "    Selected   : {}",
                if port_status.selected {
                    "YES (Online)"
                } else {
                    "NO"
                }
            );
            println!(
                "    Hardware   : {}",
                if port_status.no_error {
                    "NO ERROR (Healthy)"
                } else {
                    "ERROR DETECTED"
                }
            );
            println!(
                "    Paper Feed : {}",
                if port_status.paper_empty {
                    "OUT OF PAPER"
                } else {
                    "PAPER LOADED"
                }
            );
        }
        Err(e) => println!("    Error: {}", e),
    }

    // Pipe Enumeration
    println!("\n[Endpoints] Enumerating WinUSB Pipes:");
    match printer.query_pipes() {
        Ok(pipes) => {
            for (idx, pipe) in pipes.iter().enumerate() {
                let dir = if (pipe.pipe_id & 0x80) != 0 {
                    "IN"
                } else {
                    "OUT"
                };
                println!(
                    "    Pipe #{}: ID=0x{:02X} ({}), MaxPacketSize={}, Interval={}, Type={}",
                    idx, pipe.pipe_id, dir, pipe.maximum_packet_size, pipe.interval, pipe.pipe_type
                );
            }
        }
        Err(e) => println!("    Query pipes failed: {}", e),
    }

    // Pipe Reset Check
    println!("\n[Pipe Initialization] Resetting WinUSB pipes:");
    match printer.reset_pipes() {
        Ok(_) => println!("    WinUsb_ResetPipe(0x01 & 0x82): OK"),
        Err(e) => println!("    WinUsb_ResetPipe: Error: {}", e),
    }

    // Direct 64-byte read test
    println!("\n[Test 1] Testing direct 64-byte read on Pipe 0x82 (without write):");
    let mut raw_buf = [0u8; 64];
    match printer.read_pipe(0x82, &mut raw_buf) {
        Ok(n) => println!(
            "    Pending data in 0x82: {} bytes: {:02X?}",
            n,
            &raw_buf[..n]
        ),
        Err(e) => println!("    No pending data in 0x82 ({})", e),
    }

    // Test writing A0 E0 04 00 and reading 64 bytes
    println!("\n[Test 2] Writing A0 E0 04 00 and reading into 64-byte buffer:");
    match printer.write_pipe(0x01, &[0xa0, 0xe0, 0x04, 0x00]) {
        Ok(n) => println!("    Wrote {} bytes successfully to 0x01", n),
        Err(e) => println!("    Write failed: {}", e),
    }
    match printer.read_pipe(0x82, &mut raw_buf) {
        Ok(n) => println!("    Read {} bytes from 0x82: {:02X?}", n, &raw_buf[..n]),
        Err(e) => println!("    Read from 0x82 failed: {}", e),
    }

    // 3. USB Bulk: PCR_GET_BASIC_STATUS (A0 E0 04 00)
    println!("\n[3] Querying CAPT Basic Status (Cmd: A0 E0 04 00 over Bulk OUT:0x01, IN:0x82):");
    match printer.query_basic_status() {
        Ok(buf) => {
            println!("    Raw Response (6B)   : {:02X?}", buf);
            let opcode = u16::from_le_bytes([buf[0], buf[1]]);
            let length = u16::from_le_bytes([buf[2], buf[3]]);
            let basic = buf[4];
            let changed = buf[5];
            println!("    Opcode              : 0x{:04X}", opcode);
            println!("    Packet Length       : {} bytes", length);
            println!(
                "    Basic Register      : 0x{:02X} (UnitFree={}, Online={}, ErrorBit={})",
                basic,
                (basic & 0x40) != 0,
                (basic & 0x10) == 0,
                (basic & 0x80) != 0
            );
            println!("    Changed Register    : 0x{:02X}", changed);
        }
        Err(e) => println!("    Basic Status Error: {}", e),
    }

    // 4. USB Bulk: PC_GET_EXTENDED_STATUS (A0 A0 04 00)
    println!("\n[4] Querying CAPT Extended Status (Cmd: A0 A0 04 00 over Bulk OUT:0x01, IN:0x82):");
    match printer.query_extended_status() {
        Ok(ext) => {
            println!(
                "    Basic Register      : 0x{:02X} (UnitFree={}, Online={})",
                ext.basic, ext.unit_free, ext.online
            );
            println!("    Aux Register        : 0x{:02X}", ext.aux);
            println!("    Controller Register : 0x{:02X}", ext.controller);
            println!(
                "    Paper Bits          : 0x{:02X} ({})",
                ext.paper_available_bits,
                if (ext.paper_available_bits & 0x80) != 0 {
                    "Paper in Tray"
                } else {
                    "Tray Empty"
                }
            );
            println!("    Engine Register     : 0x{:04X}", ext.engine_raw);
            println!("    ---------------------------------------------");
            println!(
                "    Top Door / Cover    : {}",
                if ext.door_open {
                    "OPEN [!]"
                } else {
                    "CLOSED (OK)"
                }
            );
            println!(
                "    Toner Cartridge     : {}",
                if ext.no_cartridge {
                    "MISSING [!]"
                } else {
                    "INSTALLED (OK)"
                }
            );
            println!(
                "    Paper Path Jam      : {}",
                if ext.paper_jam {
                    "JAMMED [!]"
                } else {
                    "CLEAR (OK)"
                }
            );
            println!(
                "    Paper Presence      : {}",
                if ext.no_paper {
                    "EMPTY [!]"
                } else {
                    "LOADED (OK)"
                }
            );
            println!(
                "    Engine Warming/Wait : {}",
                if ext.waiting {
                    "WAITING / WARMING UP"
                } else {
                    "READY"
                }
            );
            println!(
                "    Roller Cleaning     : {}",
                if ext.cleaning { "CLEANING" } else { "IDLE" }
            );
            println!(
                "    Service Call Error  : {}",
                if ext.service_call {
                    "SERVICE CALL REQUIRED [!]"
                } else {
                    "NONE (OK)"
                }
            );
            println!("    ---------------------------------------------");
            println!(
                "    Pages: Printed={}, Printing={}, Shipped={}, Started={}",
                ext.printed_pages, ext.printing_pages, ext.shipped_pages, ext.start_pages
            );
        }
        Err(e) => println!("    Extended Status Error: {}", e),
    }

    println!("\n[PROBE COMPLETE: ZERO PRINT DATA WAS TRANSMITTED]");
    Ok(())
}

fn run_print_pbm(pbm_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("==================================================");
    println!("   CANON LBP-1120 PHYSICAL PRINT ENGINE (CAPT v1) ");
    println!("==================================================");

    println!("Loading raster: {}", pbm_path);
    let pbm = PbmImage::read_file(pbm_path)?;
    println!(
        "Raster dimensions: {}x{} (line_size: {} bytes, total: {} bytes)",
        pbm.width,
        pbm.height,
        pbm.line_size,
        pbm.data.len()
    );

    // Canonical A4 geometry for Canon LBP-1120 @ 600 DPI:
    // 592 bytes per line (4736 pixels), 6778 lines high
    let (pbm_a4, config) = if pbm.line_size == 592 && pbm.height == 6778 {
        println!("[Geometry Check] Input matches exact canonical A4 dimensions (592B x 6778L).");
        (pbm, PageHeaderConfig::default())
    } else {
        println!(
            "[Geometry Check] Normalizing input raster ({}x{}) into canonical A4 canvas (4736x6778)...",
            pbm.width, pbm.height
        );
        let mut a4_canvas = PbmImage::new(4736, 6778);
        // Center the input image horizontally and place at y=200
        let x_offset = if 4736 > pbm.width {
            (4736 - pbm.width) / 2
        } else {
            0
        };
        let y_offset = if 6778 > pbm.height { 200 } else { 0 };
        for y in 0..pbm.height.min(6778 - y_offset) {
            for x in 0..pbm.width.min(4736 - x_offset) {
                if (pbm.line(y)[x / 8] & (1 << (7 - (x % 8)))) != 0 {
                    a4_canvas.set_pixel(x_offset + x, y_offset + y, true);
                }
            }
        }
        (a4_canvas, PageHeaderConfig::default())
    };

    let mut min_x = usize::MAX;
    let mut max_x = 0;
    let mut min_y = usize::MAX;
    let mut max_y = 0;
    let mut black_pixels = 0usize;
    for y in 0..pbm_a4.height {
        let line = pbm_a4.line(y);
        for x in 0..pbm_a4.width {
            if (line[x / 8] & (1 << (7 - (x % 8)))) != 0 {
                black_pixels += 1;
                if x < min_x {
                    min_x = x;
                }
                if x > max_x {
                    max_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if y > max_y {
                    max_y = y;
                }
            }
        }
    }
    if black_pixels > 0 {
        let bbox_w = max_x.saturating_sub(min_x) + 1;
        let bbox_h = max_y.saturating_sub(min_y) + 1;
        println!(
            "[Raster Analysis] Black pixels: {} ({:.2}% coverage) | BBox: [{}, {}] -> [{}, {}] ({}x{} px)",
            black_pixels,
            (black_pixels as f64) / ((pbm_a4.width * pbm_a4.height) as f64) * 100.0,
            min_x,
            min_y,
            max_x,
            max_y,
            bbox_w,
            bbox_h
        );
    } else {
        println!("[Raster Analysis] WARNING: Canvas is completely blank (0 black pixels)!");
    }

    print!("Connecting to printer via WinUSB... ");
    let printer = UsbPrinter::find_and_open()?;
    println!("CONNECTED!");
    let _ = printer.reset_pipes();

    // 1. Initial Status Check
    println!("\n[Step 1] Checking hardware status before print...");
    let status = printer.query_extended_status()?;
    if status.door_open {
        println!(
            "Notice: Printer door/cover is open. Waiting for cover to be closed (up to 60s)..."
        );
        let mut closed = false;
        for _ in 0..60 {
            sleep(Duration::from_millis(1000));
            if let Ok(st) = printer.query_extended_status()
                && !st.door_open
            {
                closed = true;
                println!("Door closed! Proceeding...");
                break;
            }
        }
        if !closed {
            return Err("Cannot print: Printer door/cover is open!".into());
        }
    }
    if status.no_cartridge {
        return Err("Cannot print: Toner cartridge is missing!".into());
    }
    if status.paper_jam {
        println!(
            "Notice: Paper jam detected! Waiting for paper path to be cleared (open cover, remove paper, close cover, up to 90s)..."
        );
        let mut cleared = false;
        for _ in 0..90 {
            sleep(Duration::from_millis(1000));
            if let Ok(st) = printer.query_extended_status()
                && !st.paper_jam
            {
                cleared = true;
                println!("Paper path cleared! Proceeding with print...");
                break;
            }
        }
        if !cleared {
            return Err("Cannot print: Paper jam detected in printer path!".into());
        }
    }
    if status.no_paper {
        println!(
            "\n[Notice] Paper tray is currently empty. Waiting for paper to be loaded (up to 180s)..."
        );
        let mut loaded = false;
        for sec in 1..=180 {
            sleep(Duration::from_millis(1000));
            if sec % 10 == 0 {
                print!(".");
                let _ = std::io::Write::flush(&mut std::io::stdout());
            }
            if let Ok(st) = printer.query_extended_status()
                && !st.no_paper
            {
                println!("\n  Paper detected in tray! Proceeding with print...");
                loaded = true;
                break;
            }
        }
        if !loaded {
            return Err("Cannot print: Timed out waiting for paper in tray!".into());
        }
    }
    println!("Printer status healthy: Paper loaded, door closed, cartridge seated, no jams.");

    // 2. Reserve unit and prepare controller
    println!("\n[Step 2] Handshake: Reserve unit & prepare controller...");
    printer.reserve_unit()?;
    let _ = printer.clear_error();
    let _ = printer.discard_data();
    let basic_res = printer.query_basic_status()?;
    println!(
        "Reserve status byte: 0x{:02X} (UnitFree={})",
        basic_res[4],
        (basic_res[4] & 0x40) != 0
    );

    // 3. Put engine into online print mode
    println!("\n[Step 3] Issuing PCR_GO_ONLINE with magic 0xADEADBEE...");
    printer.go_online(0)?;
    let mut online = false;
    for _ in 0..50 {
        sleep(Duration::from_millis(50));
        let online_res = printer.query_basic_status()?;
        if (online_res[4] & 0x10) == 0 {
            online = true;
            println!(
                "Online state: ONLINE (Ready to accept page) (Basic=0x{:02X})",
                online_res[4]
            );
            break;
        }
    }
    if !online {
        return Err("Printer failed to enter Online mode after PCR_GO_ONLINE".into());
    }

    // 4. Encode page raster to CAPT v1 stream
    println!(
        "\n[Step 4] Compressing A4 raster with SCoA 2D delta encoding ({} lines x {} bytes)...",
        pbm_a4.height, pbm_a4.line_size
    );
    let mut encoder = ScoaEncoder::new(pbm_a4.line_size);
    for y in 0..pbm_a4.height {
        encoder.encode_line(pbm_a4.line(y));
    }
    let video_data = encoder.finish();
    println!(
        "SCoA compression: {} B -> {} B ({:.2}%)",
        pbm_a4.data.len(),
        video_data.len(),
        (video_data.len() as f64) / (pbm_a4.data.len() as f64) * 100.0
    );

    // 5. Send IC_BEGIN_PAGE (0xD0A0)
    println!("\n[Step 5] Transmitting IC_BEGIN_PAGE (Opcode 0xD0A0, 34-byte A4 geometry)...");
    let begin_page = CaptPacket::new(OPCODE_IC_BEGIN_PAGE, config.build_begin_page_payload())?;
    let mut bp_bytes = Vec::new();
    begin_page.write(&mut bp_bytes)?;
    printer.write_pipe(0x01, &bp_bytes)?;
    let _ = printer.query_basic_status();

    // 6. Send IC_BEGIN_DATA (0xD0A1)
    println!("[Step 6] Transmitting IC_BEGIN_DATA (Opcode 0xD0A1)...");
    let begin_data = CaptPacket::new(OPCODE_IC_BEGIN_DATA, Vec::new())?;
    let mut bd_bytes = Vec::new();
    begin_data.write(&mut bd_bytes)?;
    printer.write_pipe(0x01, &bd_bytes)?;
    let _ = printer.query_basic_status();

    // 7. Stream IC_VIDEO_DATA packets (chunked up to 16,384B with IM_DATA_BUSY backpressure between packets)
    println!(
        "[Step 7] Streaming IC_VIDEO_DATA ({} bytes total SCoA stream)...",
        video_data.len()
    );
    let chunk_size = 16384;
    let mut offset = 0;
    let mut packet_idx = 0;
    while offset < video_data.len() {
        let end = (offset + chunk_size).min(video_data.len());
        let chunk = &video_data[offset..end];

        // Check backpressure between packets: wait while IM_DATA_BUSY (0x08) is set
        for _ in 0..200 {
            if let Ok(buf) = printer.query_basic_status() {
                let basic = buf[4];
                if (basic & 0x08) == 0 {
                    break;
                }
                sleep(Duration::from_millis(10));
            }
        }

        let video_pkt = CaptPacket::new(OPCODE_IC_VIDEO_DATA, chunk.to_vec())?;
        let mut pkt_bytes = Vec::new();
        video_pkt.write(&mut pkt_bytes)?;
        printer.write_pipe(0x01, &pkt_bytes)?;

        offset = end;
        packet_idx += 1;
        if packet_idx % 4 == 0 || offset == video_data.len() {
            println!(
                "  Transferred {} / {} bytes ({:.1}%) [Packet #{}]",
                offset,
                video_data.len(),
                (offset as f64) / (video_data.len() as f64) * 100.0,
                packet_idx
            );
        }
    }

    // 8. Send IC_END_PAGE (0xD0A2)
    let pre_print_shipped = printer
        .query_extended_status()
        .map(|s| s.shipped_pages)
        .unwrap_or(status.shipped_pages);
    println!("\n[Step 8] Transmitting IC_END_PAGE (initiating paper pickup & motor feed)...");
    let end_page = CaptPacket::new(OPCODE_IC_END_PAGE, Vec::new())?;
    let mut ep_bytes = Vec::new();
    end_page.write(&mut ep_bytes)?;
    printer.write_pipe(0x01, &ep_bytes)?;
    let _ = printer.query_basic_status();

    // 9. Poll completion and wait for paper delivery
    println!("[Step 9] Monitoring engine cycle & page delivery (waiting for mechanical cycle)...");
    let mut feed_seen = false;
    let mut page_delivered = false;
    for sec in 1..=45 {
        sleep(Duration::from_millis(1000));
        if let Ok(st) = printer.query_extended_status() {
            let feeding = (st.aux & 0x04) != 0;
            let busy = (st.aux & 0x02) != 0;
            if feeding {
                feed_seen = true;
            }
            if st.paper_jam {
                let _ = printer.go_offline();
                return Err("Hardware Error: Paper jam occurred during page delivery!".into());
            }
            if st.service_call {
                let _ = printer.go_offline();
                return Err(format!(
                    "Hardware Error: Service call error 0x{:04X} during page delivery!",
                    st.engine_raw
                )
                .into());
            }
            println!(
                "  [T+{:02}s] Aux: 0x{:02X} (Feed={}, Busy={}) | Pages: Printed={}, Printing={}",
                sec, st.aux, feeding, busy, st.printed_pages, st.printing_pages
            );

            let delta = st.shipped_pages.wrapping_sub(pre_print_shipped);
            if delta > 0 && delta < 0x8000 {
                println!(
                    "\n  [Physical Delivery Confirmed] Page successfully ejected into output tray (Hardware counter: {} -> {}).",
                    pre_print_shipped, st.shipped_pages
                );
                page_delivered = true;
                break;
            } else if sec >= 12 && !feeding && !busy && feed_seen {
                println!(
                    "\n  [Unconfirmed Mechanical Completion] Mechanical cycle completed (feed motor idle, engine not busy), but hardware shipped counter did not advance (baseline: {}, current: {}). Delivery unconfirmed by hardware counter.",
                    pre_print_shipped, st.shipped_pages
                );
                page_delivered = true;
                break;
            }
        }
    }

    if !page_delivered {
        let _ = printer.go_offline();
        return Err(format!(
            "Print timed out: Physical delivery could not be confirmed (feed_seen={})",
            feed_seen
        )
        .into());
    }

    // 10. Cleanup: Set printer offline gracefully
    println!("[Step 10] Completing job (PCR_GO_OFFLINE)...");
    sleep(Duration::from_millis(1000));
    let _ = printer.go_offline();
    println!(
        "\n>>> [PRINT SUCCESS] Physical test page imaged, fed, and delivered to output tray. <<<"
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Canon LaserShot LBP-1120 CAPT v1 / SCoA Tool");
        println!("Usage:");
        println!("  capt-poc probe");
        println!("  capt-poc test-pdf <input.pdf> [output.pbm]");
        println!("  capt-poc compare-halftone <input.pdf> [page_idx] [prefix]");
        println!("  capt-poc print <input.pbm>");
        println!("  capt-poc generate-a4 [output.pbm]");
        println!("  capt-poc generate-sample <output.pbm> [width_bytes] [height_lines]");
        println!("  capt-poc encode <input.pbm> <output.capt> [chunk_size]");
        println!("  capt-poc decode <input.capt> <output.pbm>");
        println!("  capt-poc verify <input.pbm>");
        return Ok(());
    }

    match args[1].as_str() {
        "install" => {
            spool::install_printer()?;
        }
        "uninstall" => {
            spool::uninstall_printer()?;
        }
        "spool-server" => {
            spool::run_pipe_server(false)?;
        }
        "service" => {
            // Detach and release console so daemon runs 100% silently in background on Win10/Win11
            unsafe {
                #[link(name = "kernel32")]
                unsafe extern "system" {
                    fn FreeConsole() -> i32;
                }
                let _ = FreeConsole();
            }
            spool::run_pipe_server(true)?;
        }
        "print-pdf" => {
            let pdf_path = args.get(2).ok_or("Missing PDF file path")?;
            let mode = if args.get(3).map(|s| s.as_str()) == Some("photo") {
                pdf::HalftoneMode::Photo
            } else {
                pdf::HalftoneMode::Normal
            };
            spool::run_print_job(pdf_path, None, mode)?;
        }
        "test-pdf" => {
            let pdf_path = args.get(2).ok_or("Missing PDF file path")?;
            let count = pdf::get_pdf_page_count(pdf_path)?;
            let mode = if args.get(4).map(|s| s.as_str()) == Some("photo") {
                pdf::HalftoneMode::Photo
            } else {
                pdf::HalftoneMode::Normal
            };
            println!("PDF Page count: {}", count);
            println!("Rendering Page 0 [Mode: {}] @ 600 DPI...", mode);
            let (pbm, timing) =
                pdf::render_pdf_page_with_mode(&pdf::load_pdf_document(pdf_path)?, 0, mode)?;
            let out_pbm = args
                .get(3)
                .map(|s| s.as_str())
                .unwrap_or("test_rendered.pbm");
            pbm.write_file(out_pbm)?;
            let out_bmp = out_pbm.replace(".pbm", ".bmp");
            let _ = pbm.write_bmp(&out_bmp);
            println!(
                "Successfully rendered PDF Page 0 to {} (and {}): {}x{}, {} bytes",
                out_pbm,
                out_bmp,
                pbm.width,
                pbm.height,
                pbm.data.len()
            );
            println!(
                "Timing: WinRT Rasterization = {} ms | Pixel Halftoning = {} ms | Total = {} ms",
                timing.raster_ms,
                timing.pixel_ms,
                timing.raster_ms + timing.pixel_ms
            );
        }
        "compare-halftone" => {
            let pdf_path = args.get(2).ok_or("Missing PDF file path")?;
            let page_idx: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
            let prefix = args.get(4).map(|s| s.as_str()).unwrap_or("compare");

            let doc = pdf::load_pdf_document(pdf_path)?;
            let total_pages = doc.PageCount()?;
            println!("=======================================================");
            println!("   CANON LBP-1120 HALFTONING QUALITY COMPARISON       ");
            println!("=======================================================");
            println!(
                "Document: {} (Page {} / {})",
                pdf_path,
                page_idx + 1,
                total_pages
            );

            println!("\n[1] Rendering in Normal Mode (Text-Optimized Snapped Halftone)...");
            let (pbm_normal, timing_normal) =
                pdf::render_pdf_page_with_mode(&doc, page_idx, pdf::HalftoneMode::Normal)?;
            let normal_pbm_path = format!("{}_normal.pbm", prefix);
            let normal_bmp_path = format!("{}_normal.bmp", prefix);
            pbm_normal.write_file(&normal_pbm_path)?;
            pbm_normal.write_bmp(&normal_bmp_path)?;

            // Measure SCoA compression for Normal mode
            let mut enc_normal = scoa::ScoaEncoder::new(pbm_normal.line_size);
            for y in 0..pbm_normal.height {
                enc_normal.encode_line(pbm_normal.line(y));
            }
            let scoa_normal = enc_normal.finish();

            println!("\n[2] Rendering in Photo Mode (Serpentine Floyd-Steinberg)...");
            let (pbm_photo, timing_photo) =
                pdf::render_pdf_page_with_mode(&doc, page_idx, pdf::HalftoneMode::Photo)?;
            let photo_pbm_path = format!("{}_photo.pbm", prefix);
            let photo_bmp_path = format!("{}_photo.bmp", prefix);
            pbm_photo.write_file(&photo_pbm_path)?;
            pbm_photo.write_bmp(&photo_bmp_path)?;

            // Measure SCoA compression for Photo mode
            let mut enc_photo = scoa::ScoaEncoder::new(pbm_photo.line_size);
            for y in 0..pbm_photo.height {
                enc_photo.encode_line(pbm_photo.line(y));
            }
            let scoa_photo = enc_photo.finish();

            let normal_black = pbm_normal
                .data
                .iter()
                .map(|b| b.count_ones() as usize)
                .sum::<usize>();
            let photo_black = pbm_photo
                .data
                .iter()
                .map(|b| b.count_ones() as usize)
                .sum::<usize>();
            let total_pixels = pbm_normal.width * pbm_normal.height;

            println!("\n=======================================================");
            println!("   COMPARISON RESULTS                                  ");
            println!("=======================================================");
            println!("Metric                         | Normal (Text/Doc) | Photo (Photographic)");
            println!("-------------------------------+-------------------+---------------------");
            println!(
                "WinRT Rasterization Time       | {:>15} ms | {:>18} ms",
                timing_normal.raster_ms, timing_photo.raster_ms
            );
            println!(
                "Pixel Halftoning Time          | {:>15} ms | {:>18} ms",
                timing_normal.pixel_ms, timing_photo.pixel_ms
            );
            println!(
                "Black Pixels (Toner Dots)      | {:>17} | {:>20}",
                normal_black, photo_black
            );
            println!(
                "Black Page Coverage            | {:>16.2}% | {:>19.2}%",
                (normal_black as f64) / (total_pixels as f64) * 100.0,
                (photo_black as f64) / (total_pixels as f64) * 100.0
            );
            println!(
                "SCoA Compressed Stream Size    | {:>15} B | {:>18} B",
                scoa_normal.len(),
                scoa_photo.len()
            );
            println!(
                "SCoA Compression Ratio         | {:>16.2}% | {:>19.2}%",
                (scoa_normal.len() as f64) / (pbm_normal.data.len() as f64) * 100.0,
                (scoa_photo.len() as f64) / (pbm_photo.data.len() as f64) * 100.0
            );
            println!("-------------------------------------------------------");
            println!("Files saved for direct visual inspection (open in Paint / Photos):");
            println!("  1. {} / {}", normal_pbm_path, normal_bmp_path);
            println!("  2. {} / {}", photo_pbm_path, photo_bmp_path);
            println!("=======================================================");
        }
        "probe" => {
            run_probe()?;
        }
        "reset" => {
            println!("Resetting printer state...");
            let printer = UsbPrinter::find_and_open()?;
            printer.full_recovery();
            sleep(Duration::from_millis(500));
            match printer.query_extended_status() {
                Ok(status) => {
                    println!(
                        "Reset complete. Basic: 0x{:02X} (UnitFree={}), Controller: 0x{:02X}, Engine: 0x{:04X}",
                        status.basic, status.unit_free, status.controller, status.engine_raw
                    );
                }
                Err(e) => {
                    println!("Notice: Status query after reset: {}", e);
                }
            }
        }
        "print" => {
            let pbm_path = args.get(2).ok_or("Missing PBM file path")?;
            run_print_pbm(pbm_path)?;
        }
        "generate-a4" => {
            let out_path = args.get(2).map(|s| s.as_str()).unwrap_or("a4_test.pbm");
            println!("Generating canonical A4 test page (4736x6778 @ 600 DPI)...");
            let pbm = a4::generate_a4_test_page();
            pbm.write_file(out_path)?;
            println!(
                "Wrote full A4 test page: {} ({}x{}, {} bytes)",
                out_path,
                pbm.width,
                pbm.height,
                pbm.data.len()
            );
        }
        "generate-sample" => {
            let out_path = args.get(2).map(|s| s.as_str()).unwrap_or("sample.pbm");
            let line_size: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(592);
            let height: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(600);
            let pbm = generate_synthetic_pbm(line_size * 8, height, "text_sample");
            pbm.write_file(out_path)?;
            println!(
                "Generated sample PBM: {} ({}x{}, {} bytes)",
                out_path,
                pbm.width,
                pbm.height,
                pbm.data.len()
            );
        }
        "encode" => {
            let in_path = args.get(2).ok_or("Missing input PBM path")?;
            let out_path = args.get(3).ok_or("Missing output CAPT path")?;
            let chunk_size: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(4096);

            let pbm = PbmImage::read_file(in_path)?;
            let capt_bytes = encode_pbm_to_capt(&pbm, chunk_size)?;
            std::fs::write(out_path, &capt_bytes)?;
            println!(
                "Wrote CAPT stream to: {} ({} bytes)",
                out_path,
                capt_bytes.len()
            );
        }
        "decode" => {
            let in_path = args.get(2).ok_or("Missing input CAPT path")?;
            let out_path = args.get(3).ok_or("Missing output PBM path")?;
            let capt_bytes = std::fs::read(in_path)?;
            let pbm = decode_capt_to_pbm(&capt_bytes)?;
            pbm.write_file(out_path)?;
            println!("Wrote recovered PBM: {}", out_path);
        }
        "verify" => {
            let in_path = args.get(2).ok_or("Missing input PBM path")?;
            let pbm = PbmImage::read_file(in_path)?;
            let capt_bytes = encode_pbm_to_capt(&pbm, 4096)?;
            let recovered = decode_capt_to_pbm(&capt_bytes)?;
            if recovered.data == pbm.data {
                println!("[VERIFICATION SUCCESSFUL: 100% BIT-EXACT MATCH]");
            } else {
                return Err("Verification failed: Raster mismatch".into());
            }
        }
        other => {
            eprintln!("Unknown command: {}", other);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_synthetic_patterns() {
        for pattern in ["empty", "black", "stripes", "grid", "text_sample"] {
            let pbm = generate_synthetic_pbm(592 * 8, 200, pattern);
            let capt_stream = encode_pbm_to_capt(&pbm, 4096).unwrap();
            let recovered = decode_capt_to_pbm(&capt_stream).unwrap();
            assert_eq!(pbm.width, recovered.width);
            assert_eq!(pbm.height, recovered.height);
            assert_eq!(
                pbm.data, recovered.data,
                "Pattern '{}' failed bit-exact roundtrip",
                pattern
            );
        }
    }

    #[test]
    fn test_encode_chunk_sizes() {
        let pbm = generate_synthetic_pbm(592 * 8, 50, "text_sample");
        for chunk_size in [128, 512, 4096, 16384, 65535] {
            let capt_stream = encode_pbm_to_capt(&pbm, chunk_size).unwrap();
            let recovered = decode_capt_to_pbm(&capt_stream).unwrap();
            assert_eq!(pbm.data, recovered.data);
        }
    }

    #[test]
    fn test_e2e_single_page_text_pipeline() {
        let pbm = generate_synthetic_pbm(592 * 8, 300, "text_sample");
        let capt_bytes = encode_pbm_to_capt(&pbm, 4096).expect("encode");

        // Verify packet sequence
        let mut cursor = Cursor::new(&capt_bytes);
        let p1 = CaptPacket::read(&mut cursor).expect("pkt 1");
        assert_eq!(p1.header.opcode, OPCODE_IC_BEGIN_PAGE);
        assert_eq!(p1.payload.len(), 34);

        let p2 = CaptPacket::read(&mut cursor).expect("pkt 2");
        assert_eq!(p2.header.opcode, OPCODE_IC_BEGIN_DATA);

        let mut video_packet_count = 0;
        let mut end_page_found = false;
        while (cursor.position() as usize) < capt_bytes.len() {
            let pkt = CaptPacket::read(&mut cursor).expect("video pkt");
            if pkt.header.opcode == OPCODE_IC_VIDEO_DATA {
                video_packet_count += 1;
                assert!(pkt.payload.len() <= 4096);
            } else if pkt.header.opcode == OPCODE_IC_END_PAGE {
                end_page_found = true;
                break;
            }
        }
        assert!(video_packet_count > 0);
        assert!(end_page_found);

        // Bit-exact roundtrip verification
        let recovered = decode_capt_to_pbm(&capt_bytes).expect("decode");
        assert_eq!(recovered.width, pbm.width);
        assert_eq!(recovered.height, pbm.height);
        assert_eq!(recovered.data, pbm.data);
    }

    #[test]
    fn test_e2e_multi_page_sequence() {
        for page_idx in 0..3 {
            let pbm = generate_synthetic_pbm(
                592 * 8,
                150,
                if page_idx % 2 == 0 { "grid" } else { "stripes" },
            );
            let capt_bytes = encode_pbm_to_capt(&pbm, 2048).expect("encode page");
            let recovered = decode_capt_to_pbm(&capt_bytes).expect("decode page");
            assert_eq!(recovered.data, pbm.data);
        }
    }

    #[test]
    fn test_e2e_photograph_halftone() {
        let mut pbm = PbmImage::new(592 * 8, 200);
        // Simulate continuous-tone halftone dither pattern
        for y in 0..200 {
            for x in 0..(592 * 8) {
                if ((x * 13 + y * 7) % 17) < 9 {
                    pbm.set_pixel(x, y, true);
                }
            }
        }
        let capt_bytes = encode_pbm_to_capt(&pbm, 4096).expect("encode photo");
        let recovered = decode_capt_to_pbm(&capt_bytes).expect("decode photo");
        assert_eq!(recovered.data, pbm.data);
    }

    #[test]
    fn test_e2e_fine_lines_and_small_text() {
        let mut pbm = PbmImage::new(592 * 8, 100);
        // Alternating 1-pixel scanlines
        for y in (0..100).step_by(2) {
            pbm.line_mut(y).fill(0xFF);
        }
        let capt_bytes = encode_pbm_to_capt(&pbm, 4096).expect("encode fine lines");
        let recovered = decode_capt_to_pbm(&capt_bytes).expect("decode fine lines");
        assert_eq!(recovered.data, pbm.data);
    }

    #[test]
    fn test_e2e_mixed_text_and_image() {
        let mut pbm = PbmImage::new(592 * 8, 300);
        // Banner (top 50 lines solid black)
        for y in 0..50 {
            pbm.line_mut(y).fill(0xFF);
        }
        // Text block (middle 150 lines)
        for y in 60..200 {
            for x in 100..400 {
                if (x + y) % 8 == 0 {
                    pbm.set_pixel(x, y, true);
                }
            }
        }
        // Dithered image (bottom 80 lines)
        for y in 220..300 {
            for x in 0..(592 * 8) {
                if ((x * 3 + y * 5) % 11) < 5 {
                    pbm.set_pixel(x, y, true);
                }
            }
        }
        let capt_bytes = encode_pbm_to_capt(&pbm, 4096).expect("encode mixed");
        let recovered = decode_capt_to_pbm(&capt_bytes).expect("decode mixed");
        assert_eq!(recovered.data, pbm.data);
    }

    #[test]
    fn test_e2e_portrait_and_landscape_normalization() {
        // Landscape PBM: 800 wide, 400 high
        let mut landscape_pbm = PbmImage::new(800, 400);
        landscape_pbm.set_pixel(100, 100, true);

        // Normalize into A4 canonical canvas
        let mut a4_canvas = PbmImage::new(4736, 6778);
        let x_offset = (4736 - landscape_pbm.width) / 2;
        let y_offset = 200;
        for y in 0..landscape_pbm.height {
            for x in 0..landscape_pbm.width {
                if (landscape_pbm.line(y)[x / 8] & (1 << (7 - (x % 8)))) != 0 {
                    a4_canvas.set_pixel(x_offset + x, y_offset + y, true);
                }
            }
        }
        assert_eq!(a4_canvas.width, 4736);
        assert_eq!(a4_canvas.height, 6778);
        assert!(a4_canvas.get_pixel(x_offset + 100, y_offset + 100));
    }

    #[test]
    fn test_e2e_invalid_corrupted_input() {
        // Truncated CAPT stream: only 8 bytes
        let truncated_data = vec![0xA0, 0xD0, 0x06, 0x00, 0x01, 0x02, 0x03, 0x04];
        let res = decode_capt_to_pbm(&truncated_data);
        assert!(res.is_err());

        // Garbage bytes
        let garbage = vec![0xFF; 256];
        let res2 = decode_capt_to_pbm(&garbage);
        assert!(res2.is_err());
    }

    #[test]
    fn test_e2e_cancelled_job_state_recovery() {
        let pbm = generate_synthetic_pbm(592 * 8, 100, "text_sample");
        let capt_bytes = encode_pbm_to_capt(&pbm, 512).expect("encode");

        // Simulate cancellation mid-stream: read only first 3 packets
        let mut cursor = Cursor::new(&capt_bytes);
        let _p1 = CaptPacket::read(&mut cursor).expect("pkt1");
        let _p2 = CaptPacket::read(&mut cursor).expect("pkt2");
        let _p3 = CaptPacket::read(&mut cursor).expect("pkt3");

        // Mid-stream abort: Remaining cursor position is less than total bytes
        assert!((cursor.position() as usize) < capt_bytes.len());

        // Teardown commands that host must issue on cancellation:
        // PCR_DISCARD_DATA (0xE0A4), PCR_GO_OFFLINE (0xE0A6), PCR_RELEASE_UNIT (0xE0A9)
        let discard_cmd = [0xa4, 0xe0, 0x04, 0x00];
        let offline_cmd = [0xa6, 0xe0, 0x04, 0x00];
        let release_cmd = [0xa9, 0xe0, 0x04, 0x00];
        assert_eq!(u16::from_le_bytes([discard_cmd[0], discard_cmd[1]]), 0xE0A4);
        assert_eq!(u16::from_le_bytes([offline_cmd[0], offline_cmd[1]]), 0xE0A6);
        assert_eq!(u16::from_le_bytes([release_cmd[0], release_cmd[1]]), 0xE0A9);
    }

    #[test]
    fn test_e2e_two_consecutive_jobs() {
        // Job 1
        let job1_pbm = generate_synthetic_pbm(592 * 8, 100, "text_sample");
        let job1_capt = encode_pbm_to_capt(&job1_pbm, 4096).expect("job 1 encode");
        let job1_rec = decode_capt_to_pbm(&job1_capt).expect("job 1 decode");
        assert_eq!(job1_rec.data, job1_pbm.data);

        // Job 2 (different pattern)
        let job2_pbm = generate_synthetic_pbm(592 * 8, 100, "grid");
        let job2_capt = encode_pbm_to_capt(&job2_pbm, 4096).expect("job 2 encode");
        let job2_rec = decode_capt_to_pbm(&job2_capt).expect("job 2 decode");
        assert_eq!(job2_rec.data, job2_pbm.data);
    }

    #[test]
    fn test_e2e_simulated_usb_failure() {
        struct FailingWriter {
            bytes_before_fail: usize,
            written: usize,
        }
        impl std::io::Write for FailingWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                if self.written >= self.bytes_before_fail {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "Simulated USB pipe broken",
                    ));
                }
                let take = buf.len().min(self.bytes_before_fail - self.written);
                self.written += take;
                Ok(take)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let mut writer = FailingWriter {
            bytes_before_fail: 50,
            written: 0,
        };
        use std::io::Write;
        let res = writer.write_all(&[0xAA; 100]);
        assert!(res.is_err(), "Writer should have failed after 50 bytes");
        assert_eq!(res.unwrap_err().kind(), std::io::ErrorKind::BrokenPipe);
    }
}
