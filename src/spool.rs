use crate::pdf::{HalftoneMode, load_pdf_document, render_pdf_page_with_mode};
use crate::protocol::{
    CaptPacket, OPCODE_IC_BEGIN_DATA, OPCODE_IC_BEGIN_PAGE, OPCODE_IC_END_PAGE,
    OPCODE_IC_VIDEO_DATA, PageHeaderConfig,
};
use crate::scoa::ScoaEncoder;
use crate::usb::UsbPrinter;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;
use windows::Win32::Foundation::{GetLastError, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{PIPE_ACCESS_INBOUND, ReadFile};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE,
    PIPE_WAIT,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::w;

pub const PIPE_NAME_NORMAL: &str = r"\\.\pipe\CanonLBP1120";
pub const PIPE_NAME_PHOTO: &str = r"\\.\pipe\CanonLBP1120_Photo";
pub const PRINTER_NAME_NORMAL: &str = "Canon LaserShot LBP-1120";
pub const PRINTER_NAME_PHOTO: &str = "Canon LaserShot LBP-1120 (Photo)";

pub struct SpoolJob {
    pub path: std::path::PathBuf,
    pub ingest_ms: u128,
    pub spool_bytes: usize,
    pub mode: HalftoneMode,
}

pub fn run_print_job(
    pdf_path: &str,
    spool_ingest_ms: Option<u128>,
    mode: HalftoneMode,
) -> Result<(), Box<dyn Error>> {
    let t_job_start = std::time::Instant::now();
    println!("\n=======================================================");
    println!("   CANON LBP-1120 SPOOL JOB PROCESSOR                ");
    println!("=======================================================");
    println!("Processing job: {} [Mode: {}]", pdf_path, mode);

    let metadata = std::fs::metadata(pdf_path)?;
    let spool_bytes = metadata.len() as usize;
    if spool_bytes == 0 {
        return Err("Cannot print: Spool file is empty (0 bytes)".into());
    }

    let ingest_ms = spool_ingest_ms.unwrap_or(0);
    println!(
        "[Stage 1: Windows Spooler Ingestion] Ingested {} bytes (Time: {} ms)",
        spool_bytes, ingest_ms
    );

    let doc = load_pdf_document(pdf_path)?;
    let page_count = doc.PageCount()?;
    println!("Document contains {} page(s).", page_count);

    if page_count == 0 {
        return Err("Cannot print: Document contains zero pages".into());
    }

    let t_hw_init = std::time::Instant::now();
    print!("Connecting to physical printer via WinUSB... ");
    let printer = UsbPrinter::find_and_open()?;
    println!("CONNECTED!");
    let _ = printer.reset_pipes();

    // Check hardware status
    println!("Checking hardware status...");
    let mut status = match printer.query_extended_status() {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "Notice: Initial status query returned ({}), resetting pipes...",
                e
            );
            printer.full_recovery();
            match printer.query_extended_status() {
                Ok(s2) => s2,
                Err(e2) => {
                    return Err(format!("Hardware status query failed (if printer locked, please power cycle 5s): {}", e2).into());
                }
            }
        }
    };

    // If unit was left in reserved/unclean state from previous session, clean it up
    if !status.unit_free {
        println!(
            "Notice: Printer unit was left reserved by a previous session; clearing and resetting session state..."
        );
        let _ = printer.clear_error();
        let _ = printer.discard_data();
        let _ = printer.go_offline();
        let _ = printer.release_unit();
        sleep(Duration::from_millis(150));
        if let Ok(st) = printer.query_extended_status() {
            status = st;
        }
    }

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
        println!("\n*******************************************************");
        println!("  [تنبيه: درج الورق فارغ]");
        println!("  درج الورق فارغ حالياً! يرجى إضافة الورق لبدء الطباعة...");
        println!("  [ATTENTION: PAPER TRAY EMPTY]");
        println!("  Paper tray is empty! Please load paper to start printing...");
        println!("*******************************************************\n");
        print!("\x07");

        let mut loaded = false;
        for sec in 1..=300 {
            sleep(Duration::from_millis(1000));
            if sec % 10 == 0 {
                print!(".");
                let _ = std::io::Write::flush(&mut std::io::stdout());
            }
            if let Ok(st) = printer.query_extended_status()
                && !st.no_paper
            {
                loaded = true;
                println!(
                    "\n\n>>> [تم إضافة الورق بنجاح!] تم الكشف عن الورق في الدرج. بدء الطباعة فوراً... <<<"
                );
                println!(">>> [PAPER LOADED] Paper detected in tray. Proceeding... <<<\n");
                let _ = printer.clear_error();
                sleep(Duration::from_millis(500));
                break;
            }
        }
        if !loaded {
            return Err("Timed out waiting for paper in tray".into());
        }
    }

    let hw_handshake_ms = t_hw_init.elapsed().as_millis();
    println!("Hardware check completed ({} ms).", hw_handshake_ms);

    let mut total_raster_ms = 0u128;
    let mut total_pixel_ms = 0u128;
    let mut total_scoa_ms = 0u128;
    let mut total_usb_ms = 0u128;
    let mut total_delivery_ms = 0u128;
    let mut total_uncompressed_bytes = 0usize;
    let mut total_compressed_bytes = 0usize;
    let mut total_usb_packets = 0usize;
    let mut total_usb_bytes = 0usize;

    for page_idx in 0..page_count {
        println!(
            "\n--- Processing Page {} / {} ---",
            page_idx + 1,
            page_count
        );

        // Check if paper ran out after printing previous page(s)
        if page_idx > 0 {
            // For subsequent pages, wait until the engine has finished cycling the previous sheet
            print!(
                "Waiting for printer engine to become ready for page {}...",
                page_idx + 1
            );
            let mut engine_ready = false;
            for _ in 0..100 {
                // up to 10s
                if let Ok(st) = printer.query_extended_status()
                    && st.unit_free
                    && !st.waiting
                    && (st.aux & 0x04) == 0
                {
                    engine_ready = true;
                    break;
                }
                sleep(Duration::from_millis(100));
            }
            if !engine_ready {
                println!(
                    "Warning: Engine still busy/waiting before page {}; proceeding with caution.",
                    page_idx + 1
                );
            } else {
                println!("READY");
            }

            if let Ok(st) = printer.query_extended_status()
                && st.no_paper
            {
                println!("\n*******************************************************");
                println!("  [تنبيه: نفد الورق من درج الطابعة]");
                println!(
                    "  نفد الورق بعد طباعة الصفحة {} من أصل {}!",
                    page_idx, page_count
                );
                println!("  يرجى إضافة الورق إلى درج الطابعة لمتابعة الطباعة...");
                println!("  [ATTENTION: PAPER TRAY EMPTY]");
                println!("  Paper ran out after page {} of {}!", page_idx, page_count);
                println!("  Please add paper to the printer tray to resume printing...");
                println!("*******************************************************\n");
                print!("\x07");

                let mut loaded = false;
                for sec in 1..=300 {
                    sleep(Duration::from_millis(1000));
                    if sec % 10 == 0 {
                        print!(".");
                        let _ = std::io::Write::flush(&mut std::io::stdout());
                    }
                    if let Ok(st2) = printer.query_extended_status()
                        && !st2.no_paper
                    {
                        loaded = true;
                        println!(
                            "\n\n>>> [تم إضافة الورق بنجاح!] استئناف طباعة الصفحة {} / {} فوراً... <<<",
                            page_idx + 1,
                            page_count
                        );
                        println!(
                            ">>> [PAPER LOADED] Resuming page {} of {}... <<<\n",
                            page_idx + 1,
                            page_count
                        );
                        let _ = printer.clear_error();
                        sleep(Duration::from_millis(500));
                        break;
                    }
                }
                if !loaded {
                    let _ = printer.go_offline();
                    let _ = printer.release_unit();
                    return Err(format!(
                        "Cannot print: Timed out waiting for paper to be added (page {}/{})",
                        page_idx + 1,
                        page_count
                    )
                    .into());
                }
            }
        }

        // Query fresh baseline shipped pages immediately before beginning this page
        let baseline_shipped = match printer.query_extended_status() {
            Ok(st) => st.shipped_pages,
            Err(_) => 0,
        };

        // Stage 2: Rasterization
        let (pbm, render_timing) = render_pdf_page_with_mode(&doc, page_idx, mode)?;
        total_raster_ms += render_timing.raster_ms;
        total_pixel_ms += render_timing.pixel_ms;
        total_uncompressed_bytes += pbm.data.len();

        println!(
            "[Stage 2: 600 DPI PDF Rasterization] Rendered in {} ms",
            render_timing.raster_ms
        );
        println!(
            "[Stage 3: Pixel Processing (1-bpp)] Converted in {} ms (Bitmap: {}x{}, {} bytes)",
            render_timing.pixel_ms,
            pbm.width,
            pbm.height,
            pbm.data.len()
        );

        // Stage 4: SCoA Compression
        let t_scoa = std::time::Instant::now();
        let mut encoder = ScoaEncoder::new(pbm.line_size);
        for y in 0..pbm.height {
            encoder.encode_line(pbm.line(y));
        }
        let video_data = encoder.finish();
        let scoa_ms = t_scoa.elapsed().as_millis();
        total_scoa_ms += scoa_ms;
        total_compressed_bytes += video_data.len();

        let ratio = (video_data.len() as f64) / (pbm.data.len() as f64) * 100.0;
        println!(
            "[Stage 4: SCoA Compression] Compressed: {} B -> {} B ({:.2}%) in {} ms",
            pbm.data.len(),
            video_data.len(),
            ratio,
            scoa_ms
        );

        // Handshake: Session initialization and entering online mode
        if page_idx == 0 {
            print!("Reserving printer unit and entering online mode... ");
            printer.reserve_unit()?;
            let _ = printer.clear_error();
            let _ = printer.discard_data();
            printer.go_online(0)?;

            let mut online = false;
            for _ in 0..50 {
                sleep(Duration::from_millis(50));
                let basic_res = printer.query_basic_status()?;
                if (basic_res[4] & 0x10) == 0 {
                    online = true;
                    break;
                }
            }
            if !online {
                let _ = printer.go_offline();
                let _ = printer.release_unit();
                return Err("Printer failed to enter Online mode for print job".into());
            }
            println!("ONLINE (Ready)");
        } else {
            // Check online state for subsequent pages; re-assert go_online only if dropped
            let basic_res = printer.query_basic_status()?;
            if (basic_res[4] & 0x10) != 0 {
                println!("Re-entering online mode for page {}...", page_idx + 1);
                printer.go_online(page_idx as u16)?;
                sleep(Duration::from_millis(100));
            }
        }

        // Stage 5: USB Transmission
        let t_usb = std::time::Instant::now();
        let mut page_usb_bytes = 0usize;
        let mut page_packets = 0usize;

        // 1. IC_BEGIN_PAGE (0xD0A0)
        let config = PageHeaderConfig::default();
        let begin_page = CaptPacket::new(OPCODE_IC_BEGIN_PAGE, config.build_begin_page_payload())?;
        let mut bp_bytes = Vec::new();
        begin_page.write(&mut bp_bytes)?;
        println!(
            "[Stage 5.1: USB Tx] IC_BEGIN_PAGE: sending {} bytes...",
            bp_bytes.len()
        );
        let n = printer.write_pipe(0x01, &bp_bytes)?;
        println!(
            "[Stage 5.1: USB Tx] IC_BEGIN_PAGE: successfully wrote {} bytes",
            n
        );
        let _ = printer.query_basic_status();
        page_usb_bytes += n;
        page_packets += 1;

        // 2. IC_BEGIN_DATA (0xD0A1)
        let begin_data = CaptPacket::new(OPCODE_IC_BEGIN_DATA, Vec::new())?;
        let mut bd_bytes = Vec::new();
        begin_data.write(&mut bd_bytes)?;
        println!(
            "[Stage 5.2: USB Tx] IC_BEGIN_DATA: sending {} bytes...",
            bd_bytes.len()
        );
        let n = printer.write_pipe(0x01, &bd_bytes)?;
        println!(
            "[Stage 5.2: USB Tx] IC_BEGIN_DATA: successfully wrote {} bytes",
            n
        );
        let _ = printer.query_basic_status();
        page_usb_bytes += n;
        page_packets += 1;

        // 3. Stream IC_VIDEO_DATA packets (chunked 4096 bytes with backpressure check)
        let chunk_size = 4096;
        let total_chunks = video_data.len().div_ceil(chunk_size);
        let mut offset = 0;
        let mut chunk_idx = 0;
        while offset < video_data.len() {
            let end = (offset + chunk_size).min(video_data.len());
            let chunk = &video_data[offset..end];

            // Flow control / backpressure check: wait while IM_DATA_BUSY (0x08) is set
            let mut busy_cleared = false;
            for _ in 0..200 {
                if let Ok(buf) = printer.query_basic_status() {
                    let basic = buf[4];
                    if (basic & 0x08) == 0 {
                        busy_cleared = true;
                        break;
                    }
                }
                sleep(Duration::from_millis(10));
            }
            if !busy_cleared {
                eprintln!(
                    "[Stage 5.3: USB Tx] Warning: Printer buffer remained busy before chunk {}/{}",
                    chunk_idx + 1,
                    total_chunks
                );
            }

            let video_pkt = CaptPacket::new(OPCODE_IC_VIDEO_DATA, chunk.to_vec())?;
            let mut pkt_bytes = Vec::new();
            video_pkt.write(&mut pkt_bytes)?;
            let n = printer.write_pipe(0x01, &pkt_bytes)?;
            page_usb_bytes += n;
            page_packets += 1;
            chunk_idx += 1;

            if chunk_idx == 1 || chunk_idx == total_chunks || chunk_idx % 4 == 0 {
                println!(
                    "  [Stage 5.3: USB Tx] IC_VIDEO_DATA chunk {}/{} ({} B sent, total {}/{} B)",
                    chunk_idx,
                    total_chunks,
                    n,
                    end,
                    video_data.len()
                );
            }

            offset = end;
        }

        // 4. IC_END_PAGE (0xD0A2)
        println!("[Stage 5.4: USB Tx] IC_END_PAGE: sending 4 bytes...");
        let end_page = CaptPacket::new(OPCODE_IC_END_PAGE, Vec::new())?;
        let mut ep_bytes = Vec::new();
        end_page.write(&mut ep_bytes)?;
        let n = printer.write_pipe(0x01, &ep_bytes)?;
        println!(
            "[Stage 5.4: USB Tx] IC_END_PAGE: successfully wrote {} bytes",
            n
        );
        let _ = printer.query_basic_status();
        page_usb_bytes += n;
        page_packets += 1;

        let usb_ms = t_usb.elapsed().as_millis();
        total_usb_ms += usb_ms;
        total_usb_packets += page_packets;
        total_usb_bytes += page_usb_bytes;

        println!(
            "[Stage 5: USB Transmission] Page {} stream transmitted: {} packets ({} bytes) in {} ms",
            page_idx + 1,
            page_packets,
            page_usb_bytes,
            usb_ms
        );

        // Stage 6: Mechanical paper ejection
        println!(
            "  [Stage 6: Monitoring Delivery] Waiting for physical paper pickup and ejection..."
        );
        let t_delivery = std::time::Instant::now();
        let mut page_delivered = false;
        let mut feed_seen = false;
        for iter in 1..=120 {
            // up to 30s with 250ms polling
            sleep(Duration::from_millis(250));
            if let Ok(st) = printer.query_extended_status() {
                let feeding = (st.aux & 0x04) != 0;
                let busy = (st.aux & 0x02) != 0;
                if feeding {
                    feed_seen = true;
                }
                if st.paper_jam {
                    let _ = printer.go_offline();
                    let _ = printer.release_unit();
                    return Err(format!(
                        "Hardware Error: Paper jam occurred during page {} delivery!",
                        page_idx + 1
                    )
                    .into());
                }
                if st.service_call {
                    let _ = printer.go_offline();
                    let _ = printer.release_unit();
                    return Err(format!(
                        "Hardware Error: Service call error 0x{:04X} during page {} delivery!",
                        st.engine_raw,
                        page_idx + 1
                    )
                    .into());
                }
                if st.shipped_pages > baseline_shipped
                    || (feed_seen && !feeding && !busy && iter >= 24)
                {
                    println!(
                        "  [Physical Delivery Confirmed] Page {}/{} successfully delivered to output tray (Shipped: {} -> {}).",
                        page_idx + 1,
                        page_count,
                        baseline_shipped,
                        st.shipped_pages
                    );
                    page_delivered = true;
                    break;
                }
            }
        }
        if !page_delivered {
            let _ = printer.go_offline();
            let _ = printer.release_unit();
            return Err(format!("Print timed out: Page {}/{} physical delivery could not be confirmed (feed_seen={})", page_idx + 1, page_count, feed_seen).into());
        }
        let delivery_ms = t_delivery.elapsed().as_millis();
        total_delivery_ms += delivery_ms;
        println!(
            "[Stage 6: Printer Mechanical Delivery] Completed in {} ms",
            delivery_ms
        );

        if page_idx + 1 < page_count {
            sleep(Duration::from_millis(500));
        }
    }

    // Complete job: take printer offline and release unit
    sleep(Duration::from_millis(250));
    let _ = printer.go_offline();
    let _ = printer.release_unit();

    // Clear Windows print queues so job disappears from "File d'attente"
    clear_all_windows_print_queues();

    let total_job_ms = t_job_start.elapsed().as_millis();
    let host_processing_ms =
        ingest_ms + total_raster_ms + total_pixel_ms + total_scoa_ms + total_usb_ms;
    let printer_hardware_ms = hw_handshake_ms + total_delivery_ms + 250;

    println!("\n=======================================================");
    println!("   CANON LBP-1120 PERFORMANCE MEASUREMENT REPORT      ");
    println!("=======================================================");
    println!(
        "1. Windows Spooler Ingestion     : {:>6} ms ({:>8} bytes)",
        ingest_ms, spool_bytes
    );
    println!(
        "2. WinRT 600 DPI Rasterization   : {:>6} ms",
        total_raster_ms
    );
    println!(
        "3. Pixel Processing (1-bpp PBM)  : {:>6} ms (Uncompressed: {} B)",
        total_pixel_ms, total_uncompressed_bytes
    );
    println!(
        "4. SCoA 2D Compression           : {:>6} ms (Compressed: {} B, {:.2}%)",
        total_scoa_ms,
        total_compressed_bytes,
        (total_compressed_bytes as f64) / (total_uncompressed_bytes as f64) * 100.0
    );
    println!(
        "5. USB Data Transmission         : {:>6} ms (Packets: {}, Sent: {} B)",
        total_usb_ms, total_usb_packets, total_usb_bytes
    );
    println!(
        "6. Printer Mechanical Delivery   : {:>6} ms (Handshake: {} ms + Eject: {} ms)",
        printer_hardware_ms, hw_handshake_ms, total_delivery_ms
    );
    println!("-------------------------------------------------------");
    println!(
        "HOST PC PROCESSING TIME          : {:>6} ms ({:5.1}%)",
        host_processing_ms,
        (host_processing_ms as f64) / (total_job_ms as f64) * 100.0
    );
    println!(
        "PRINTER HARDWARE WAIT TIME       : {:>6} ms ({:5.1}%)",
        printer_hardware_ms,
        (printer_hardware_ms as f64) / (total_job_ms as f64) * 100.0
    );
    println!("TOTAL END-TO-END JOB TIME        : {:>6} ms", total_job_ms);
    println!("=======================================================");

    println!(
        "\n>>> [PRINT SUCCESS] All {} page(s) successfully imaged, fed, and physically delivered to output tray. <<<",
        page_count
    );
    Ok(())
}

fn listen_on_pipe(
    pipe_name: &str,
    mode: HalftoneMode,
    tx: std::sync::mpsc::Sender<SpoolJob>,
    silent: bool,
) {
    let pipe_wide: Vec<u16> = pipe_name.encode_utf16().chain(std::iter::once(0)).collect();
    let temp_dir = env::temp_dir();

    let h_pipe = unsafe {
        CreateNamedPipeW(
            windows::core::PCWSTR(pipe_wide.as_ptr()),
            PIPE_ACCESS_INBOUND,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            255,
            65536,
            65536,
            0,
            None,
        )
    };

    if h_pipe == INVALID_HANDLE_VALUE {
        let err = unsafe { GetLastError() };
        if !silent {
            eprintln!(
                "Fatal: CreateNamedPipeW failed for '{}' (error {}).",
                pipe_name, err.0
            );
        }
        return;
    }

    loop {
        let connected = unsafe { ConnectNamedPipe(h_pipe, None) };
        let last_err = unsafe { GetLastError() };

        if connected.is_ok() || last_err.0 == 535 {
            // 535 = ERROR_PIPE_CONNECTED
            let mut buf = [0u8; 65536];
            let mut first_bytes_read = 0u32;
            let first_res =
                unsafe { ReadFile(h_pipe, Some(&mut buf), Some(&mut first_bytes_read), None) };

            if first_res.is_err() || first_bytes_read == 0 {
                if !silent {
                    println!(
                        "\n[Spool Monitor - {}] Spooler connected but sent 0 bytes. Connection closed.",
                        mode
                    );
                }
                unsafe {
                    let _ = DisconnectNamedPipe(h_pipe);
                }
                continue;
            }

            let t_ingest_start = std::time::Instant::now();
            if !silent {
                println!(
                    "\n>>> Print stream incoming on '{}' [{}]! Ingesting data...",
                    pipe_name, mode
                );
            }

            static JOB_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
            let seq = JOB_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let pid = std::process::id();
            let job_path = temp_dir.join(format!("lbp1120_job_{}_{}_{}.pdf", pid, ts, seq));

            let mut file = match File::create(&job_path) {
                Ok(f) => f,
                Err(e) => {
                    if !silent {
                        eprintln!("Failed to create temp spool file: {}", e);
                    }
                    unsafe {
                        let _ = DisconnectNamedPipe(h_pipe);
                    }
                    continue;
                }
            };

            let mut write_err = false;
            if let Err(e) = file.write_all(&buf[..first_bytes_read as usize]) {
                if !silent {
                    eprintln!("Failed writing to temp spool file: {}", e);
                }
                write_err = true;
            }
            let mut total_bytes = first_bytes_read as usize;

            loop {
                let mut bytes_read = 0u32;
                let res = unsafe { ReadFile(h_pipe, Some(&mut buf), Some(&mut bytes_read), None) };

                if res.is_err() || bytes_read == 0 {
                    break;
                }

                if !write_err && let Err(e) = file.write_all(&buf[..bytes_read as usize]) {
                    if !silent {
                        eprintln!("Failed writing to temp spool file: {}", e);
                    }
                    write_err = true;
                }
                total_bytes += bytes_read as usize;
            }

            if let Err(e) = file.flush() {
                if !silent {
                    eprintln!("Failed flushing temp spool file: {}", e);
                }
                write_err = true;
            }
            drop(file);

            unsafe {
                let _ = DisconnectNamedPipe(h_pipe);
            }

            if write_err {
                let _ = std::fs::remove_file(&job_path);
                continue;
            }

            let ingest_ms = t_ingest_start.elapsed().as_millis();
            if !silent {
                println!(
                    "Ingested {} bytes in {} ms [{}]",
                    total_bytes, ingest_ms, mode
                );
            }

            if total_bytes < 32 {
                let _ = std::fs::remove_file(&job_path);
                continue;
            }

            if !is_pdf_file(&job_path) {
                if !silent {
                    let mut preview = [0u8; 16];
                    let preview_len = if let Ok(mut f) = File::open(&job_path) {
                        use std::io::Read;
                        f.read(&mut preview).unwrap_or(0)
                    } else {
                        0
                    };
                    eprintln!(
                        "[Spool Monitor] Rejected non-PDF stream on '{}' ({} bytes, magic: {:02X?}).",
                        pipe_name,
                        total_bytes,
                        &preview[..preview_len]
                    );
                }
                let _ = std::fs::remove_file(&job_path);
                continue;
            }

            if let Err(e) = tx.send(SpoolJob {
                path: job_path,
                ingest_ms,
                spool_bytes: total_bytes,
                mode,
            }) {
                eprintln!("[Spool Monitor] Failed to enqueue print job: {}", e);
            }
        } else {
            sleep(Duration::from_millis(50));
        }
    }
}

pub fn run_pipe_server(silent: bool) -> Result<(), Box<dyn Error>> {
    // Enforce single-instance execution via a named Win32 Mutex
    let mutex_name = w!(r"Local\CanonLBP1120_SpoolServer_Mutex");
    let _h_mutex = match unsafe { CreateMutexW(None, true, mutex_name) } {
        Ok(h) => {
            if unsafe { GetLastError() }.0 == 183 {
                // 183 = ERROR_ALREADY_EXISTS
                if !silent {
                    println!(
                        "[Spool Server] Another instance of Canon LBP-1120 spool server is already running. Exiting cleanly."
                    );
                }
                return Ok(());
            }
            h
        }
        Err(e) => {
            if !silent {
                eprintln!("[Spool Server] Failed to create instance mutex: {}", e);
            }
            return Err(e.into());
        }
    };

    if !silent {
        println!("=======================================================");
        println!("   CANON LBP-1120 DUAL-MODE PRINT SPOOL SERVER        ");
        println!("=======================================================");
        println!("Listening on ports:");
        println!("  - Normal Mode (Text/Document) : {}", PIPE_NAME_NORMAL);
        println!("  - Photo Mode (Photographs)    : {}", PIPE_NAME_PHOTO);
        println!("Waiting for Windows print jobs (Ctrl+C to exit)...");
    }

    let (tx, rx) = std::sync::mpsc::channel::<SpoolJob>();

    // Dedicated print worker thread: prints jobs sequentially so named pipe is immediately free for next job
    let worker_silent = silent;
    std::thread::spawn(move || {
        // Ensure worker thread is initialized for COM/WinRT MTA (required on Windows 10 & 11)
        unsafe {
            #[link(name = "ole32")]
            unsafe extern "system" {
                fn CoInitializeEx(pvReserved: *mut std::ffi::c_void, dwCoInit: u32) -> i32;
            }
            let _ = CoInitializeEx(std::ptr::null_mut(), 0); // 0 = COINIT_MULTITHREADED
        }

        while let Ok(job) = rx.recv() {
            let path_str = job.path.to_string_lossy().to_string();
            if !worker_silent {
                println!(
                    "[Spool Worker] Dequeued job: {} ({} bytes, ingested in {} ms, Mode: {})",
                    path_str, job.spool_bytes, job.ingest_ms, job.mode
                );
            }
            match run_print_job(&path_str, Some(job.ingest_ms), job.mode) {
                Ok(_) => {
                    let printer_name = match job.mode {
                        HalftoneMode::Normal => PRINTER_NAME_NORMAL,
                        HalftoneMode::Photo => PRINTER_NAME_PHOTO,
                    };
                    clear_completed_job(printer_name);
                }
                Err(e) => {
                    if !worker_silent {
                        eprintln!("Print job failed: {}", e);
                    }
                    clear_stuck_windows_jobs();
                }
            }
            // Cleanup temp file
            let _ = std::fs::remove_file(&job.path);
        }
    });

    // Spawn listener thread for Photo pipe
    let tx_photo = tx.clone();
    std::thread::spawn(move || {
        listen_on_pipe(PIPE_NAME_PHOTO, HalftoneMode::Photo, tx_photo, silent);
    });

    // Run listener for Normal pipe on this thread
    listen_on_pipe(PIPE_NAME_NORMAL, HalftoneMode::Normal, tx, silent);
    Ok(())
}

pub fn install_printer() -> Result<(), Box<dyn Error>> {
    println!("=======================================================");
    println!("   CANON LBP-1120 NATIVE DUAL-DRIVER INSTALLER        ");
    println!("=======================================================");

    let exe_path = env::current_exe()?.to_string_lossy().replace(r"\\?\", "");

    use std::os::windows::process::CommandExt;

    // 1. Create Normal Named Pipe printer port
    println!(
        "[1/6] Registering Normal printer port '{}'...",
        PIPE_NAME_NORMAL
    );
    let port1_cmd = format!(
        "if (-not (Get-PrinterPort -Name '{0}' -ErrorAction SilentlyContinue)) {{ Add-PrinterPort -Name '{0}' }}",
        PIPE_NAME_NORMAL
    );
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", &port1_cmd])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output();

    // 2. Add Normal Printer
    println!("[2/6] Registering '{}' in Windows...", PRINTER_NAME_NORMAL);
    let print1_cmd = format!(
        "if (-not (Get-Printer -Name '{0}' -ErrorAction SilentlyContinue)) {{ Add-Printer -Name '{0}' -DriverName 'Microsoft Print to PDF' -PortName '{1}' }}",
        PRINTER_NAME_NORMAL, PIPE_NAME_NORMAL
    );
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", &print1_cmd])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output();

    // 3. Create Photo Named Pipe printer port
    println!(
        "[3/6] Registering Photo printer port '{}'...",
        PIPE_NAME_PHOTO
    );
    let port2_cmd = format!(
        "if (-not (Get-PrinterPort -Name '{0}' -ErrorAction SilentlyContinue)) {{ Add-PrinterPort -Name '{0}' }}",
        PIPE_NAME_PHOTO
    );
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", &port2_cmd])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output();

    // 4. Add Photo Printer
    println!("[4/6] Registering '{}' in Windows...", PRINTER_NAME_PHOTO);
    let print2_cmd = format!(
        "if (-not (Get-Printer -Name '{0}' -ErrorAction SilentlyContinue)) {{ Add-Printer -Name '{0}' -DriverName 'Microsoft Print to PDF' -PortName '{1}' }}",
        PRINTER_NAME_PHOTO, PIPE_NAME_PHOTO
    );
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", &print2_cmd])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output();

    // 5. Register startup entry
    println!("[5/6] Registering background service in Windows startup...");
    let reg_cmd = format!(
        "Set-ItemProperty -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run' -Name 'CanonLBP1120' -Value '\"{0}\" service'",
        exe_path
    );
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", &reg_cmd])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output();

    // 6. Launch background service now
    println!("[6/6] Starting dual-mode background print spool listener...");
    let _ = Command::new(&exe_path)
        .arg("service")
        .creation_flags(0x08000008) // DETACHED_PROCESS | CREATE_NO_WINDOW
        .spawn();

    println!("\n>>> DUAL-DRIVER INSTALLATION COMPLETE! <<<");
    println!("1. '{}' (Normal: Text & Documents)", PRINTER_NAME_NORMAL);
    println!(
        "2. '{}' (Photo: Photographs & Halftones)",
        PRINTER_NAME_PHOTO
    );
    println!("Both printers are now installed and ready in the Windows Print Dialog.");
    Ok(())
}

pub fn uninstall_printer() -> Result<(), Box<dyn Error>> {
    println!("=======================================================");
    println!("   CANON LBP-1120 NATIVE DRIVER UNINSTALLER           ");
    println!("=======================================================");

    use std::os::windows::process::CommandExt;

    println!("[1/5] Removing printer '{}'...", PRINTER_NAME_NORMAL);
    let _ = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Remove-Printer -Name '{}' -ErrorAction SilentlyContinue",
                PRINTER_NAME_NORMAL
            ),
        ])
        .creation_flags(0x08000000)
        .output();

    println!("[2/5] Removing printer '{}'...", PRINTER_NAME_PHOTO);
    let _ = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Remove-Printer -Name '{}' -ErrorAction SilentlyContinue",
                PRINTER_NAME_PHOTO
            ),
        ])
        .creation_flags(0x08000000)
        .output();

    println!("[3/5] Removing printer port '{}'...", PIPE_NAME_NORMAL);
    let _ = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Remove-PrinterPort -Name '{}' -ErrorAction SilentlyContinue",
                PIPE_NAME_NORMAL
            ),
        ])
        .creation_flags(0x08000000)
        .output();

    println!("[4/5] Removing printer port '{}'...", PIPE_NAME_PHOTO);
    let _ = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Remove-PrinterPort -Name '{}' -ErrorAction SilentlyContinue",
                PIPE_NAME_PHOTO
            ),
        ])
        .creation_flags(0x08000000)
        .output();

    println!("[5/5] Removing startup entry...");
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", "Remove-ItemProperty -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run' -Name 'CanonLBP1120' -ErrorAction SilentlyContinue"])
        .creation_flags(0x08000000)
        .output();

    // Kill any running service instance
    let _ = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-Process capt-poc -ErrorAction SilentlyContinue | Stop-Process -Force",
        ])
        .creation_flags(0x08000000)
        .output();

    println!("\nDriver uninstalled successfully.");
    Ok(())
}

pub fn clear_completed_job(printer_name: &str) {
    let clean_cmd = format!(
        "Get-PrintJob -PrinterName '{0}' -ErrorAction SilentlyContinue | Select-Object -First 1 | Remove-PrintJob -ErrorAction SilentlyContinue",
        printer_name
    );
    use std::os::windows::process::CommandExt;
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", &clean_cmd])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output();
}

pub fn clear_stuck_windows_jobs() {
    let clean_cmd = format!(
        "Get-PrintJob -PrinterName '{0}' -ErrorAction SilentlyContinue | Where-Object {{ $_.JobStatus -match 'Error|Deleting' }} | Remove-PrintJob -ErrorAction SilentlyContinue; Get-PrintJob -PrinterName '{1}' -ErrorAction SilentlyContinue | Where-Object {{ $_.JobStatus -match 'Error|Deleting' }} | Remove-PrintJob -ErrorAction SilentlyContinue",
        PRINTER_NAME_NORMAL, PRINTER_NAME_PHOTO
    );
    use std::os::windows::process::CommandExt;
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", &clean_cmd])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output();
}

pub fn clear_all_windows_print_queues() {
    clear_windows_print_queue(PRINTER_NAME_NORMAL);
    clear_windows_print_queue(PRINTER_NAME_PHOTO);
}

pub fn clear_windows_print_queue(printer_name: &str) {
    let clean_cmd = format!(
        "Get-PrintJob -PrinterName '{0}' -ErrorAction SilentlyContinue | Remove-PrintJob -ErrorAction SilentlyContinue",
        printer_name
    );
    use std::os::windows::process::CommandExt;
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", &clean_cmd])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output();
}

pub fn is_pdf_buffer(buf: &[u8]) -> bool {
    buf.windows(4).any(|w| w == b"%PDF")
}

pub fn is_pdf_file(path: &std::path::Path) -> bool {
    if let Ok(mut f) = File::open(path) {
        use std::io::Read;
        let mut buf = [0u8; 1024];
        if let Ok(n) = f.read(&mut buf) {
            return is_pdf_buffer(&buf[..n]);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_pdf_buffer_exact() {
        assert!(is_pdf_buffer(b"%PDF-1.4\n"));
    }

    #[test]
    fn test_is_pdf_buffer_with_utf8_bom() {
        let mut data = vec![0xEF, 0xBB, 0xBF];
        data.extend_from_slice(b"%PDF-1.7\n");
        assert!(is_pdf_buffer(&data));
    }

    #[test]
    fn test_is_pdf_buffer_with_leading_whitespace() {
        let data = b"   \r\n\t%PDF-2.0\n";
        assert!(is_pdf_buffer(data));
    }

    #[test]
    fn test_is_pdf_buffer_non_pdf() {
        assert!(!is_pdf_buffer(b"GIF89a"));
        assert!(!is_pdf_buffer(b"\x89PNG\r\n\x1a\n"));
        assert!(!is_pdf_buffer(b"MZ\x90\x00\x03\x00"));
        assert!(!is_pdf_buffer(b"Plain text document"));
    }
}
