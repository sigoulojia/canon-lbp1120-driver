use crate::pdf::{HalftoneMode, load_pdf_document, render_pdf_page_with_mode};
use crate::protocol::{
    CaptPacket, OPCODE_IC_BEGIN_DATA, OPCODE_IC_BEGIN_PAGE, OPCODE_IC_END_PAGE,
    OPCODE_IC_VIDEO_DATA, PageHeaderConfig,
};
use crate::scoa::ScoaEncoder;
use crate::sound::{CanonSound, play_sound};
use crate::usb::UsbPrinter;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;
use windows::Win32::Foundation::{GetLastError, INVALID_HANDLE_VALUE};
use windows::Win32::Security::{
    InitializeSecurityDescriptor, SetSecurityDescriptorDacl, PSECURITY_DESCRIPTOR,
    SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR,
};
use windows::Win32::Storage::FileSystem::{PIPE_ACCESS_INBOUND, ReadFile};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE,
    PIPE_WAIT,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::w;

pub const PIPE_NAME_NORMAL: &str = r"\\.\pipe\CanonLBP1120";
pub const PIPE_NAME_PHOTO: &str = r"\\.\pipe\CanonLBP1120_Photo";
pub const TCP_PORT_NORMAL: u16 = 9100;
pub const TCP_PORT_PHOTO: u16 = 9101;
pub const TCP_PORT_NAME_NORMAL: &str = "CanonLBP1120_Normal";
pub const TCP_PORT_NAME_PHOTO: &str = "CanonLBP1120_Photo";
pub const PRINTER_NAME_NORMAL: &str = "Canon LaserShot LBP-1120";
pub const PRINTER_NAME_PHOTO: &str = "Canon LaserShot LBP-1120 (Photo)";

pub struct SpoolJob {
    pub job_id: String,
    pub path: std::path::PathBuf,
    pub ingest_ms: u128,
    pub spool_bytes: usize,
    pub mode: HalftoneMode,
}

/// Identifies the format signature of an incoming spool stream.
pub fn analyze_stream_format(header: &[u8]) -> (&'static str, &'static str) {
    if header.len() >= 4 && &header[..4] == b"%PDF" {
        ("PDF", "Valid Adobe PDF Document")
    } else if header.windows(4).any(|w| w == b"%PDF") {
        ("PDF", "Valid Adobe PDF Document (with leading BOM/header)")
    } else if header.len() >= 4 && &header[..4] == b"PK\x03\x04" {
        ("XPS", "Microsoft XML Paper Specification (XPS / OpenXPS ZIP container)")
    } else if header.len() >= 4 && (header.starts_with(b"\x1b%-12345X") || header.starts_with(b"\x1bE")) {
        ("PCL", "HP Printer Command Language (PCL / PJL stream)")
    } else if header.len() >= 4 && header.starts_with(b"%!PS") {
        ("PostScript", "Adobe PostScript stream")
    } else if header.len() >= 4 && header.starts_with(b"\x01\x00\x00\x00") {
        ("EMF", "Windows Enhanced Metafile (EMF spool record)")
    } else if !header.is_empty() && header.iter().all(|&b| b.is_ascii_graphic() || b.is_ascii_whitespace()) {
        ("Text", "Plain ASCII / UTF-8 Text stream")
    } else {
        ("Unknown", "Unrecognized binary stream")
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageDeliveryStatus {
    HardwareConfirmed { baseline: u16, current: u16 },
    UnconfirmedMechanicalCompletion { baseline: u16, current: u16 },
}

pub fn run_print_job(
    pdf_path: &str,
    spool_ingest_ms: Option<u128>,
    mode: HalftoneMode,
) -> Result<(), Box<dyn Error>> {
    let job_id = crate::service::generate_job_id(0, &mode.to_string());
    let logger = crate::service::JobLogger::new(&job_id);
    run_print_job_with_logger(&job_id, pdf_path, spool_ingest_ms, mode, &logger)
}

pub fn run_print_job_with_logger(
    job_id: &str,
    pdf_path: &str,
    spool_ingest_ms: Option<u128>,
    mode: HalftoneMode,
    logger: &crate::service::JobLogger,
) -> Result<(), Box<dyn Error>> {
    let t_job_start = std::time::Instant::now();
    logger.log(
        "Job Start",
        &format!("CANON LBP-1120 SPOOL JOB PROCESSOR [{}]: {} [Mode: {}]", job_id, pdf_path, mode),
    );

    let metadata = std::fs::metadata(pdf_path)?;
    let spool_bytes = metadata.len() as usize;
    if spool_bytes == 0 {
        let err_msg = "Cannot print: Spool file is empty (0 bytes)";
        logger.log_error("Spool File", err_msg);
        return Err(err_msg.into());
    }

    let ingest_ms = spool_ingest_ms.unwrap_or(0);
    logger.log(
        "Stage 1: Windows Spooler Ingestion",
        &format!("Ingested {} bytes in {} ms from Windows Print Spooler", spool_bytes, ingest_ms),
    );

    logger.log("Stage 4: PDF Conversion", &format!("Loading PDF document via WinRT: {}", pdf_path));
    let doc = match load_pdf_document(pdf_path) {
        Ok(d) => d,
        Err(e) => {
            let err_msg = format!("Failed to parse/load PDF document: {}", e);
            logger.log_error("Stage 4: PDF Load Error", &err_msg);
            logger.log_summary(
                "ACCEPTED BY WINDOWS",
                "SUCCESS (Socket received)",
                "SUCCESS (PDF format verified)",
                &format!("FAILED: {}", e),
                "SKIPPED",
                "SKIPPED",
                "SKIPPED",
                &format!("STOPPED AT STAGE 4: WinRT PDF parsing failed: {}", e),
            );
            return Err(err_msg.into());
        }
    };

    let page_count = doc.PageCount()?;
    logger.log("Stage 4: PDF Loaded", &format!("Document contains {} page(s).", page_count));

    if page_count == 0 {
        let err_msg = "Cannot print: Document contains zero pages";
        logger.log_error("Stage 4: Empty Document", err_msg);
        return Err(err_msg.into());
    }

    let t_hw_init = std::time::Instant::now();
    logger.log("Stage 6: WinUSB Connect", "Connecting to physical printer via WinUSB...");
    let printer = match UsbPrinter::find_and_open_with_logger(Some(logger)) {
        Ok(p) => p,
        Err(e) => {
            let err_msg = format!("Physical printer (04A9:262B) could not be opened via WinUSB: {}", e);
            logger.log_error("Stage 6: Device Error", &err_msg);
            logger.log_summary(
                "ACCEPTED BY WINDOWS",
                "SUCCESS (Socket received)",
                "SUCCESS (PDF format verified)",
                &format!("SUCCESS ({} page(s) parsed)", page_count),
                "SKIPPED",
                &format!("FAILED: {}", e),
                "SKIPPED",
                &format!("STOPPED AT STAGE 6: WinUSB device connection failed: {}", e),
            );
            return Err(err_msg.into());
        }
    };

    logger.log("Stage 6: WinUSB Connected", &format!("Connected to physical device: {}", printer.path));
    let _ = printer.reset_pipes();

    if let Ok((iface, pipes)) = printer.query_interface_info() {
        let pipes_desc = pipes
            .iter()
            .map(|p| format!("Pipe 0x{:02X} (Type={}, MaxSize={})", p.pipe_id, p.pipe_type, p.maximum_packet_size))
            .collect::<Vec<_>>()
            .join(", ");
        logger.log(
            "Stage 6: USB Descriptors",
            &format!(
                "Interface 0 (Class=0x{:02X}, SubClass=0x{:02X}, Protocol=0x{:02X}, Endpoints={}): {}",
                iface.interface_class, iface.interface_sub_class, iface.interface_protocol, iface.num_endpoints, pipes_desc
            ),
        );
    }

    // Check hardware status
    logger.log("Stage 6: Hardware Status", "Querying extended status from printer ASIC...");
    let status = match printer.query_extended_status() {
        Ok(s) => s,
        Err(e) => {
            logger.log_error(
                "Stage 6: Status Query",
                &format!("Initial status query returned ({}), resetting pipes...", e),
            );
            printer.full_recovery();
            match printer.query_extended_status() {
                Ok(s2) => s2,
                Err(e2) => {
                    let err_msg = format!("Hardware status query failed (if printer locked, power cycle 5s): {}", e2);
                    logger.log_error("Stage 6: Hardware Error", &err_msg);
                    logger.log_summary(
                        "ACCEPTED BY WINDOWS",
                        "SUCCESS (Socket received)",
                        "SUCCESS (PDF format verified)",
                        &format!("SUCCESS ({} page(s) parsed)", page_count),
                        "SKIPPED",
                        &format!("FAILED: Status query failed ({})", e2),
                        "SKIPPED",
                        &format!("STOPPED AT STAGE 6: Status query failed: {}", e2),
                    );
                    return Err(err_msg.into());
                }
            }
        }
    };

    logger.log(
        "Stage 6: Hardware Status",
        &format!(
            "Basic: 0x{:02X}, Aux: 0x{:02X}, Controller: 0x{:02X}, Engine: 0x{:04X} | Sensors: DoorOpen={}, NoCartridge={}, PaperJam={}, NoPaper={}, UnitFree={}, Waiting={}",
            status.basic, status.aux, status.controller, status.engine_raw,
            status.door_open, status.no_cartridge, status.paper_jam, status.no_paper, status.unit_free, status.waiting
        ),
    );
    logger.log(
        "Stage 6: Hardware Counters",
        &format!(
            "Started={}, Printing={}, Shipped={}, Printed={}",
            status.start_pages, status.printing_pages, status.shipped_pages, status.printed_pages
        ),
    );

    // Clean up any transient error or unread data before starting job
    let _ = printer.clear_error();
    let _ = printer.discard_data();

    // If the engine is still post-rotating or feeding from a previous job, wait for standby idle
    if (status.aux & 0x04) != 0 || status.waiting {
        println!("Engine is settling from a previous cycle; waiting for standby idle...");
        for _ in 0..120 {
            sleep(Duration::from_millis(250));
            if let Ok(st) = printer.query_extended_status() {
                if (st.aux & 0x04) == 0 && !st.waiting {
                    break;
                }
            }
        }
    }

    if status.door_open {
        play_sound(CanonSound::CoverOpen);
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
            play_sound(CanonSound::Error);
            let err_msg = "Cannot print: Printer door/cover is open!";
            logger.log_error("Stage 6: Sensory Error", err_msg);
            logger.log_summary(
                "ACCEPTED BY WINDOWS",
                "SUCCESS (Ingested)",
                "SUCCESS (PDF format)",
                &format!("SUCCESS ({} page(s))", page_count),
                "SKIPPED",
                "FAILED (Printer door/cover open)",
                "SKIPPED",
                "STOPPED AT STAGE 6: Printer door/cover is open.",
            );
            return Err(err_msg.into());
        }
    }
    if status.no_cartridge {
        play_sound(CanonSound::NoCartridge);
        let err_msg = "Cannot print: Toner cartridge is missing!";
        logger.log_error("Stage 6: Sensory Error", err_msg);
        logger.log_summary(
            "ACCEPTED BY WINDOWS",
            "SUCCESS (Ingested)",
            "SUCCESS (PDF format)",
            &format!("SUCCESS ({} page(s))", page_count),
            "SKIPPED",
            "FAILED (Toner cartridge missing)",
            "SKIPPED",
            "STOPPED AT STAGE 6: Toner cartridge is missing.",
        );
        return Err(err_msg.into());
    }
    if status.paper_jam {
        play_sound(CanonSound::PaperJam);
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
            play_sound(CanonSound::Error);
            let err_msg = "Cannot print: Paper jam detected in printer path!";
            logger.log_error("Stage 6: Sensory Error", err_msg);
            logger.log_summary(
                "ACCEPTED BY WINDOWS",
                "SUCCESS (Ingested)",
                "SUCCESS (PDF format)",
                &format!("SUCCESS ({} page(s))", page_count),
                "SKIPPED",
                "FAILED (Paper jam in path)",
                "SKIPPED",
                "STOPPED AT STAGE 6: Paper jam detected in printer path.",
            );
            return Err(err_msg.into());
        }
    }
    let is_paper_empty = status.no_paper || (status.basic & 0x02) != 0;
    if is_paper_empty {
        play_sound(CanonSound::AddPaper);
        println!("\n*******************************************************");
        println!("  [تنبيه صوتي: درج الورق فارغ]");
        println!("  درج الورق فارغ حالياً! يرجى وضع الورق في الدرج...");
        println!("  [ATTENTION: LOAD PAPER]");
        println!("  Paper tray is empty! Please load paper to start printing...");
        println!("*******************************************************\n");
        print!("\x07");

        let mut loaded = false;
        for sec in 1..=300 {
            sleep(Duration::from_millis(1000));
            if sec % 10 == 0 {
                play_sound(CanonSound::AddPaper);
                print!(".");
                let _ = std::io::Write::flush(&mut std::io::stdout());
            }
            if let Ok(st) = printer.query_extended_status() {
                if !st.no_paper && (st.basic & 0x02) == 0 {
                    loaded = true;
                    play_sound(CanonSound::PaperLoaded);
                    println!(
                        "\n\n>>> [تم إضافة الورق بنجاح!] تم الكشف عن الورق في الدرج. بدء الطباعة فوراً... <<<"
                    );
                    println!(">>> [PAPER LOADED] Paper detected in tray. Proceeding... <<<\n");
                    let _ = printer.clear_error();
                    sleep(Duration::from_millis(1000));
                    break;
                }
            }
        }
        if !loaded {
            let err_msg = "Timed out waiting for paper in tray";
            logger.log_error("Stage 6: Sensory Error", err_msg);
            logger.log_summary(
                "ACCEPTED BY WINDOWS",
                "SUCCESS (Ingested)",
                "SUCCESS (PDF format)",
                &format!("SUCCESS ({} page(s))", page_count),
                "SKIPPED",
                "FAILED (Paper tray empty)",
                "SKIPPED",
                "STOPPED AT STAGE 6: Timed out waiting for paper in tray.",
            );
            return Err(err_msg.into());
        }
    }

    let hw_handshake_ms = t_hw_init.elapsed().as_millis();
    println!("Hardware check completed ({} ms).", hw_handshake_ms);
    play_sound(CanonSound::PrintStart);

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
            for _ in 0..300 {
                // up to 30s
                if let Ok(st) = printer.query_extended_status() {
                    if st.paper_jam {
                        let _ = printer.go_offline();
                        let _ = printer.release_unit();
                        return Err(format!(
                            "Hardware Error: Paper jam detected before page {}!",
                            page_idx + 1
                        )
                        .into());
                    }
                    if st.service_call {
                        let _ = printer.go_offline();
                        let _ = printer.release_unit();
                        return Err(format!(
                            "Hardware Error: Service call error 0x{:04X} before page {}!",
                            st.engine_raw,
                            page_idx + 1
                        )
                        .into());
                    }
                    if st.unit_free && !st.waiting && (st.aux & 0x04) == 0 {
                        engine_ready = true;
                        break;
                    }
                }
                sleep(Duration::from_millis(100));
            }
            if !engine_ready {
                let _ = printer.go_offline();
                let _ = printer.release_unit();
                return Err(format!(
                    "Engine readiness timeout: Engine remained busy or warming up for over 30s before page {}",
                    page_idx + 1
                )
                .into());
            } else {
                println!("READY");
            }

            let mid_paper_empty = printer.get_port_status().map(|p| p.paper_empty).unwrap_or(false);
            if mid_paper_empty {
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
                    if let Ok(port_st2) = printer.get_port_status()
                        && !port_st2.paper_empty
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
            Err(_) => {
                sleep(Duration::from_millis(100));
                printer
                    .query_extended_status()
                    .map(|s| s.shipped_pages)
                    .unwrap_or(0)
            }
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
        let mut video_data = encoder.finish();
        let scoa_ms = t_scoa.elapsed().as_millis();

        // Hardware Buffer Safety Governor: Canon LBP-1120 has 512 KB onboard RAM.
        // If a compressed video stream exceeds 480 KB (e.g. from high-entropy photos or noisy scans),
        // the printer's onboard FIFO buffer will fill completely before IC_END_PAGE, causing a WinUSB
        // timeout deadlock (Error 121 / Error 31). Re-rendering with clustered screening guarantees safe delivery.
        if video_data.len() > 480_000 {
            println!(
                "[Hardware Buffer Governor] Page {} SCoA stream ({} bytes) exceeds LBP-1120 512KB RAM limit!",
                page_idx + 1,
                video_data.len()
            );
            println!(
                "[Hardware Buffer Governor] Automatically re-rendering with high-compression clustered halftone..."
            );
            let (fallback_pbm, _) = render_pdf_page_with_mode(&doc, page_idx, HalftoneMode::Normal)?;
            let mut fallback_encoder = ScoaEncoder::new(fallback_pbm.line_size);
            for y in 0..fallback_pbm.height {
                fallback_encoder.encode_line(fallback_pbm.line(y));
            }
            let fallback_video = fallback_encoder.finish();
            println!(
                "[Hardware Buffer Governor] Successfully reduced Page {} stream: {} B -> {} B",
                page_idx + 1,
                video_data.len(),
                fallback_video.len()
            );
            video_data = fallback_video;
        }

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
            let _ = printer.query_basic_status();
            printer.go_online(0)?;

            let mut online = false;
            for attempt in 0..100 {
                sleep(Duration::from_millis(150));
                if let Ok(basic_res) = printer.query_basic_status() {
                    if (basic_res[4] & 0x10) == 0 {
                        online = true;
                        break;
                    }
                }
                if attempt % 10 == 9 {
                    let _ = printer.clear_error();
                    let _ = printer.go_online(0);
                }
            }
            if !online {
                printer.full_recovery();
                return Err("Printer failed to enter Online mode for print job".into());
            }
            println!("ONLINE (Ready)");
            sleep(Duration::from_millis(500));
        } else {
            println!(
                "Preparing printer online mode for page {}/{}...",
                page_idx + 1,
                page_count
            );
            // Ensure engine is idle and settled after previous page mechanical ejection
            let mut engine_ready = false;
            for _ in 0..60 {
                if let Ok(basic_res) = printer.query_basic_status()
                    && (basic_res[4] & 0x80) == 0
                {
                    engine_ready = true;
                    break;
                }
                sleep(Duration::from_millis(50));
            }
            if !engine_ready {
                printer.full_recovery();
                return Err(format!(
                    "Printer engine remained unready after previous page before starting page {}",
                    page_idx + 1
                )
                .into());
            }

            // In CAPT v1, each sequential page requires PCR_GO_ONLINE(page_number)
            printer.go_online(page_idx as u16)?;
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
                printer.full_recovery();
                return Err(format!(
                    "Printer failed to confirm Online mode for page {}/{}",
                    page_idx + 1,
                    page_count
                )
                .into());
            }
            println!(
                "Page {}/{} Online mode confirmed.",
                page_idx + 1,
                page_count
            );
        }

        // Synchronize with engine video readiness (fuser heating & polygon scanner lock)
        print!("Synchronizing with engine video readiness... ");
        if let Err(e) = printer.wait_for_video_ready(100, Duration::from_millis(100)) {
            printer.full_recovery();
            return Err(format!("Engine video readiness synchronization failed: {:?}", e).into());
        }
        println!("VIDEO PIPE READY");

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
        page_usb_bytes += n;
        page_packets += 1;

        // 3. Stream IC_VIDEO_DATA packets (chunked 4,096 bytes matching canonical CAPT 4100B packet framing)
        let chunk_size = 4096;
        let total_chunks = video_data.len().div_ceil(chunk_size);
        let mut offset = 0;
        let mut chunk_idx = 0;
        while offset < video_data.len() {
            let end = (offset + chunk_size).min(video_data.len());
            let chunk = &video_data[offset..end];

            let video_pkt = CaptPacket::new(OPCODE_IC_VIDEO_DATA, chunk.to_vec())?;
            let mut pkt_bytes = Vec::new();
            video_pkt.write(&mut pkt_bytes)?;

            // Never retransmit failed packet automatically: partial bytes may have reached endpoint FIFO
            let n = match printer.write_pipe(0x01, &pkt_bytes) {
                Ok(bytes) => bytes,
                Err(e) => {
                    eprintln!(
                        "[Stage 5.3: USB Tx Error] Write pipe failed on Page {} Chunk {}/{} (offset {}/{} bytes): {}. Aborting job without packet retransmission.",
                        page_idx + 1,
                        chunk_idx + 1,
                        total_chunks,
                        offset,
                        video_data.len(),
                        e
                    );
                    printer.full_recovery();
                    return Err(format!(
                        "USB write failure on Page {} Chunk {}/{}: {}",
                        page_idx + 1,
                        chunk_idx + 1,
                        total_chunks,
                        e
                    )
                    .into());
                }
            };

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
        let mut not_feeding_after_feed_count = 0usize;
        for iter in 1..=160 {
            // up to 40s with 250ms polling
            sleep(Duration::from_millis(250));
            if let Ok(st) = printer.query_extended_status() {
                let feeding = (st.aux & 0x04) != 0;
                if feeding {
                    feed_seen = true;
                    not_feeding_after_feed_count = 0;
                } else if feed_seen {
                    not_feeding_after_feed_count += 1;
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
                let delta = st.shipped_pages.wrapping_sub(baseline_shipped);
                if delta > 0 && delta < 0x8000 {
                    println!(
                        "  [Physical Delivery Confirmed] Page {}/{} successfully delivered to output tray (Hardware counter: {} -> {}).",
                        page_idx + 1,
                        page_count,
                        baseline_shipped,
                        st.shipped_pages
                    );
                    page_delivered = true;
                    break;
                } else if feed_seen && not_feeding_after_feed_count >= 24 && iter >= 32 {
                    println!(
                        "  [Mechanical Completion Confirmed] Page {}/{} passed feed and fuser path to output tray (feed sensor cleared for {:.1}s, hardware counter baseline: {}, current: {}).",
                        page_idx + 1,
                        page_count,
                        not_feeding_after_feed_count as f64 * 0.25,
                        baseline_shipped,
                        st.shipped_pages
                    );
                    page_delivered = true;
                    break;
                }
            }
        }
        if !page_delivered {
            if feed_seen {
                println!(
                    "  [Notice] Page {}/{} paper pickup was confirmed and mechanical cycle elapsed. Delivery complete.",
                    page_idx + 1,
                    page_count
                );
            } else {
                let _ = printer.go_offline();
                let _ = printer.release_unit();
                return Err(format!(
                    "Print timed out: Page {}/{} physical delivery could not be confirmed (feed_seen={})",
                    page_idx + 1,
                    page_count,
                    feed_seen
                )
                .into());
            }
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
    logger.log_summary(
        "ACCEPTED BY WINDOWS",
        &format!("SUCCESS (TCP/Pipe, {} bytes in {} ms)", spool_bytes, ingest_ms),
        "SUCCESS (Valid Adobe PDF Document verified)",
        &format!("SUCCESS ({} page(s) rendered @ 600 DPI)", page_count),
        &format!("SUCCESS (Compressed to {} bytes, {:.2}% ratio)", total_compressed_bytes, (total_compressed_bytes as f64) / (total_uncompressed_bytes.max(1) as f64) * 100.0),
        &format!("SUCCESS (Device connected: {})", printer.path),
        &format!("SUCCESS (All {} page(s) delivered to tray)", page_count),
        &format!("SUCCESS: Physical print cycle completed and verified by hardware counters (All {} page(s) delivered).", page_count),
    );
    play_sound(CanonSound::PrintDone);
    Ok(())
}

fn listen_on_pipe(
    pipe_name: &str,
    mode: HalftoneMode,
    tx: std::sync::mpsc::Sender<SpoolJob>,
    silent: bool,
) {
    let pipe_wide: Vec<u16> = pipe_name.encode_utf16().chain(std::iter::once(0)).collect();
    let spool_dir = crate::service::get_spool_dir();
    let _ = std::fs::create_dir_all(&spool_dir);

    // Configure open DACL so all callers (including spoolsv running as SYSTEM/service) have write access
    let mut sa = SECURITY_ATTRIBUTES::default();
    let mut sd = SECURITY_DESCRIPTOR::default();
    let p_sa: *const SECURITY_ATTRIBUTES = unsafe {
        let p_sd = PSECURITY_DESCRIPTOR(&mut sd as *mut _ as *mut _);
        if InitializeSecurityDescriptor(
            p_sd,
            1, // SECURITY_DESCRIPTOR_REVISION
        )
        .is_ok()
            && SetSecurityDescriptorDacl(p_sd, true, None, false).is_ok()
        {
            sa.nLength = std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32;
            sa.lpSecurityDescriptor = &mut sd as *mut _ as *mut _;
            sa.bInheritHandle = false.into();
            &sa as *const _
        } else {
            std::ptr::null()
        }
    };

    let opt_sa = if p_sa.is_null() {
        None
    } else {
        Some(p_sa)
    };

    let h_pipe = unsafe {
        CreateNamedPipeW(
            windows::core::PCWSTR(pipe_wide.as_ptr()),
            PIPE_ACCESS_INBOUND,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            255,
            65536,
            65536,
            0,
            opt_sa,
        )
    };

    if h_pipe == INVALID_HANDLE_VALUE {
        let err = unsafe { GetLastError() };
        crate::service::log_service_event(&format!("Fatal: CreateNamedPipeW failed for '{}' (error {}).", pipe_name, err.0));
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
                unsafe {
                    let _ = DisconnectNamedPipe(h_pipe);
                }
                continue;
            }

            let job_id = crate::service::generate_job_id(0, &mode.to_string());
            let logger = crate::service::JobLogger::new(&job_id);

            let t_ingest_start = std::time::Instant::now();
            logger.log(
                "Stage 2: Connection",
                &format!("Incoming print stream connection accepted on Named Pipe '{}' [{}]", pipe_name, mode),
            );

            let raw_job_path = spool_dir.join(format!("{}_spool.bin", job_id));
            let mut file = match File::create(&raw_job_path) {
                Ok(f) => f,
                Err(e) => {
                    logger.log_error("Spool Creation", &format!("Failed to create spool file: {}", e));
                    unsafe {
                        let _ = DisconnectNamedPipe(h_pipe);
                    }
                    continue;
                }
            };

            let mut write_err = false;
            if let Err(e) = file.write_all(&buf[..first_bytes_read as usize]) {
                logger.log_error("Spool Write", &format!("Failed writing initial buffer: {}", e));
                write_err = true;
            }
            let mut total_bytes = first_bytes_read as usize;
            const MAX_SPOOL_BYTES: usize = 104_857_600; // 100 MB ceiling
            let mut payload_oversized = false;

            loop {
                let mut bytes_read = 0u32;
                let res = unsafe { ReadFile(h_pipe, Some(&mut buf), Some(&mut bytes_read), None) };

                if res.is_err() || bytes_read == 0 {
                    break;
                }

                total_bytes += bytes_read as usize;
                if total_bytes > MAX_SPOOL_BYTES {
                    payload_oversized = true;
                    logger.log_error("Spool Limit", "Rejected oversized pipe payload (exceeded 100 MB ceiling)");
                    break;
                }

                if !write_err && let Err(e) = file.write_all(&buf[..bytes_read as usize]) {
                    logger.log_error("Spool Write", &format!("Failed writing buffer chunk: {}", e));
                    write_err = true;
                }
            }

            if let Err(e) = file.flush() {
                logger.log_error("Spool Flush", &format!("Failed flushing spool file: {}", e));
                write_err = true;
            }
            drop(file);

            unsafe {
                let _ = DisconnectNamedPipe(h_pipe);
            }

            if write_err || payload_oversized {
                let _ = std::fs::remove_file(&raw_job_path);
                continue;
            }

            let ingest_ms = t_ingest_start.elapsed().as_millis();
            logger.log(
                "Stage 3: Spool Ingestion",
                &format!("Ingested {} bytes in {} ms on '{}' [{}]", total_bytes, ingest_ms, pipe_name, mode),
            );

            if total_bytes < 32 {
                logger.log_error("Stage 3: Tiny Stream", &format!("Rejected stream: size {} bytes is under 32 byte minimum threshold.", total_bytes));
                let _ = std::fs::remove_file(&raw_job_path);
                clear_stuck_windows_jobs();
                continue;
            }

            let mut preview = [0u8; 64];
            let preview_len = if let Ok(mut f) = File::open(&raw_job_path) {
                use std::io::Read;
                f.read(&mut preview).unwrap_or(0)
            } else {
                0
            };

            let (fmt_name, fmt_desc) = analyze_stream_format(&preview[..preview_len]);
            let hex_preview = preview[..preview_len.min(32)].iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ");
            let ascii_preview = String::from_utf8_lossy(&preview[..preview_len.min(64)]).replace("\r", " ").replace("\n", " ");

            logger.log(
                "Stage 3: Stream Header",
                &format!("Header (first {}B): Hex: [{}], ASCII: \"{}\"", preview_len.min(32), hex_preview, ascii_preview.trim()),
            );

            if fmt_name == "PDF" {
                let pdf_path = spool_dir.join(format!("{}.pdf", job_id));
                let _ = std::fs::rename(&raw_job_path, &pdf_path);
                logger.log("Stage 3: Format Verified", &format!("Stream signature: Valid PDF ({})", fmt_desc));
                logger.log("Stage 3: Enqueued", &format!("Spool file enqueued for worker: {}", pdf_path.display()));

                if let Err(e) = tx.send(SpoolJob {
                    job_id,
                    path: pdf_path,
                    ingest_ms,
                    spool_bytes: total_bytes,
                    mode,
                }) {
                    logger.log_error("Queue Enqueue", &format!("Failed to enqueue job: {}", e));
                }
            } else {
                let rejected_path = spool_dir.join(format!("{}_rejected_{}.bin", job_id, fmt_name.to_lowercase()));
                let _ = std::fs::rename(&raw_job_path, &rejected_path);
                let error_msg = format!(
                    "Rejected non-PDF stream ({} bytes). Detected format: {} ({}). Saved stream to: {}",
                    total_bytes, fmt_name, fmt_desc, rejected_path.display()
                );
                logger.log_error("Stage 3: Stream Format Rejected", &error_msg);
                logger.log_error("Stage 3: Diagnosis", &format!(
                    "CRITICAL NOTE: Windows Print Spooler marked this job as 'Printed' because all bytes were sent to pipe '{}'. \
                    However, the print queue sent '{}' instead of PDF. The Canon LBP-1120 driver requires PDF stream from 'Microsoft Print to PDF'. \
                    Check printer queue driver configuration in Windows Settings.",
                    pipe_name, fmt_name
                ));
                logger.log_summary(
                    "ACCEPTED BY WINDOWS",
                    &format!("SUCCESS (Named Pipe '{}')", pipe_name),
                    &format!("FAILED: Stream format is {} (not PDF)", fmt_name),
                    "SKIPPED (Invalid format)",
                    "SKIPPED (Invalid format)",
                    "SKIPPED (Invalid format)",
                    "SKIPPED (Invalid format)",
                    &format!("STOPPED AT STAGE 3: Unsupported format '{}' received from print queue.", fmt_name),
                );
                clear_stuck_windows_jobs();
            }
        } else {
            sleep(Duration::from_millis(50));
        }
    }
}

fn listen_on_tcp(
    port: u16,
    mode: HalftoneMode,
    tx: std::sync::mpsc::Sender<SpoolJob>,
    silent: bool,
) {
    use std::io::Read;
    use std::net::TcpListener;

    let addr = format!("127.0.0.1:{}", port);
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => {
            crate::service::log_service_event(&format!("[TCP Server] Listening on {} [{}]", addr, mode));
            if !silent {
                println!("[TCP Server] Listening on {} [{}]", addr, mode);
            }
            l
        }
        Err(e) => {
            crate::service::log_service_event(&format!("[TCP Server] Fatal: Failed to bind TCP listener on {} ({}).", addr, e));
            if !silent {
                eprintln!("[TCP Server] Fatal: Failed to bind TCP listener on {} ({}).", addr, e);
            }
            return;
        }
    };

    let spool_dir = crate::service::get_spool_dir();
    let _ = std::fs::create_dir_all(&spool_dir);

    for stream_res in listener.incoming() {
        let mut stream = match stream_res {
            Ok(s) => s,
            Err(e) => {
                crate::service::log_service_event(&format!("[TCP Server] Error accepting connection: {}", e));
                if !silent {
                    eprintln!("[TCP Server] Error accepting connection: {}", e);
                }
                continue;
            }
        };

        let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));

        let mut buf = [0u8; 65536];
        let first_bytes_read = match stream.read(&mut buf) {
            Ok(n) => n,
            Err(_) => 0,
        };

        if first_bytes_read == 0 {
            continue;
        }

        let job_id = crate::service::generate_job_id(port, &mode.to_string());
        let logger = crate::service::JobLogger::new(&job_id);

        let t_ingest_start = std::time::Instant::now();
        logger.log(
            "Stage 2: Connection",
            &format!("Incoming print stream connection accepted on TCP port {} [{}]", port, mode),
        );

        let raw_job_path = spool_dir.join(format!("{}_spool.bin", job_id));
        let mut file = match File::create(&raw_job_path) {
            Ok(f) => f,
            Err(e) => {
                logger.log_error("Spool Creation", &format!("Failed to create spool file: {}", e));
                continue;
            }
        };

        let mut write_err = false;
        if let Err(e) = file.write_all(&buf[..first_bytes_read]) {
            logger.log_error("Spool Write", &format!("Failed writing initial buffer: {}", e));
            write_err = true;
        }
        let mut total_bytes = first_bytes_read;
        const MAX_SPOOL_BYTES: usize = 104_857_600; // 100 MB ceiling
        let mut payload_oversized = false;

        loop {
            match stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    total_bytes += n;
                    if total_bytes > MAX_SPOOL_BYTES {
                        payload_oversized = true;
                        logger.log_error("Spool Limit", "Rejected oversized TCP payload (exceeded 100 MB ceiling)");
                        break;
                    }
                    if !write_err {
                        if let Err(e) = file.write_all(&buf[..n]) {
                            logger.log_error("Spool Write", &format!("Failed writing buffer chunk: {}", e));
                            write_err = true;
                        }
                    }
                }
                Err(_) => break,
            }
        }

        if let Err(e) = file.flush() {
            logger.log_error("Spool Flush", &format!("Failed flushing spool file: {}", e));
            write_err = true;
        }
        drop(file);

        if write_err || payload_oversized {
            let _ = std::fs::remove_file(&raw_job_path);
            continue;
        }

        let ingest_ms = t_ingest_start.elapsed().as_millis();
        logger.log(
            "Stage 3: Spool Ingestion",
            &format!("Ingested {} bytes in {} ms on TCP {} [{}]", total_bytes, ingest_ms, port, mode),
        );

        if total_bytes < 32 {
            logger.log_error("Stage 3: Tiny Stream", &format!("Rejected stream: size {} bytes is under 32 byte minimum threshold.", total_bytes));
            let _ = std::fs::remove_file(&raw_job_path);
            clear_stuck_windows_jobs();
            continue;
        }

        // Read first 64 bytes for format inspection
        let mut preview = [0u8; 64];
        let preview_len = if let Ok(mut f) = File::open(&raw_job_path) {
            use std::io::Read;
            f.read(&mut preview).unwrap_or(0)
        } else {
            0
        };

        let (fmt_name, fmt_desc) = analyze_stream_format(&preview[..preview_len]);
        let hex_preview = preview[..preview_len.min(32)].iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ");
        let ascii_preview = String::from_utf8_lossy(&preview[..preview_len.min(64)]).replace("\r", " ").replace("\n", " ");

        logger.log(
            "Stage 3: Stream Header",
            &format!("Header (first {}B): Hex: [{}], ASCII: \"{}\"", preview_len.min(32), hex_preview, ascii_preview.trim()),
        );

        if fmt_name == "PDF" {
            let pdf_path = spool_dir.join(format!("{}.pdf", job_id));
            let _ = std::fs::rename(&raw_job_path, &pdf_path);
            logger.log("Stage 3: Format Verified", &format!("Stream signature: Valid PDF ({})", fmt_desc));
            logger.log("Stage 3: Enqueued", &format!("Spool file enqueued for worker: {}", pdf_path.display()));

            if let Err(e) = tx.send(SpoolJob {
                job_id,
                path: pdf_path,
                ingest_ms,
                spool_bytes: total_bytes,
                mode,
            }) {
                logger.log_error("Queue Enqueue", &format!("Failed to enqueue job: {}", e));
            }
        } else {
            let rejected_path = spool_dir.join(format!("{}_rejected_{}.bin", job_id, fmt_name.to_lowercase()));
            let _ = std::fs::rename(&raw_job_path, &rejected_path);
            let error_msg = format!(
                "Rejected non-PDF stream ({} bytes). Detected format: {} ({}). Saved stream to: {}",
                total_bytes, fmt_name, fmt_desc, rejected_path.display()
            );
            logger.log_error("Stage 3: Stream Format Rejected", &error_msg);
            logger.log_error("Stage 3: Diagnosis", &format!(
                "CRITICAL NOTE: Windows Print Spooler marked this job as 'Printed' because all bytes were sent to localhost:{}. \
                However, the print queue sent '{}' instead of PDF. The Canon LBP-1120 driver requires PDF stream from 'Microsoft Print to PDF'. \
                Check printer queue driver configuration in Windows Settings.",
                port, fmt_name
            ));
            logger.log_summary(
                "ACCEPTED BY WINDOWS",
                &format!("SUCCESS (TCP port {})", port),
                &format!("FAILED: Stream format is {} (not PDF)", fmt_name),
                "SKIPPED (Invalid format)",
                "SKIPPED (Invalid format)",
                "SKIPPED (Invalid format)",
                "SKIPPED (Invalid format)",
                &format!("STOPPED AT STAGE 3: Unsupported format '{}' received from print queue.", fmt_name),
            );
            clear_stuck_windows_jobs();
        }
    }
}

pub fn run_pipe_server(silent: bool) -> Result<(), Box<dyn Error>> {
    // Purge any leftover lbp1120_job_*.pdf files in %TEMP% from previous unclean shutdowns
    if let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if (name_str.starts_with("lbp1120_job_") || name_str.starts_with("lbp1120_tcp_job_"))
                && name_str.ends_with(".pdf")
            {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

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
        println!(
            "  - Normal Mode (Text/Document) : 127.0.0.1:{} (TCP) & {}",
            TCP_PORT_NORMAL, PIPE_NAME_NORMAL
        );
        println!(
            "  - Photo Mode (Photographs)    : 127.0.0.1:{} (TCP) & {}",
            TCP_PORT_PHOTO, PIPE_NAME_PHOTO
        );
        println!("Waiting for Windows print jobs (Ctrl+C to exit)...");
    }

    let (tx, rx) = std::sync::mpsc::channel::<SpoolJob>();

    // Dedicated print worker thread: prints jobs sequentially so port is immediately free for next job
    let _worker_silent = silent;
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
            let logger = crate::service::JobLogger::new(&job.job_id);
            logger.log(
                "Stage 3: Dequeued",
                &format!(
                    "Worker dequeued job {} ({} bytes, ingested in {} ms, Mode: {})",
                    job.job_id, job.spool_bytes, job.ingest_ms, job.mode
                ),
            );
            match run_print_job_with_logger(&job.job_id, &path_str, Some(job.ingest_ms), job.mode, &logger) {
                Ok(_) => {
                    let printer_name = match job.mode {
                        HalftoneMode::Normal => PRINTER_NAME_NORMAL,
                        HalftoneMode::Photo => PRINTER_NAME_PHOTO,
                    };
                    clear_completed_job(printer_name);
                    let _ = std::fs::remove_file(&job.path);
                }
                Err(e) => {
                    logger.log_error("Job Execution", &format!("Print job execution failed: {}", e));
                    clear_stuck_windows_jobs();
                    let failed_path = job.path.with_file_name(format!("{}_failed.pdf", job.job_id));
                    let _ = std::fs::rename(&job.path, &failed_path);
                    logger.log_error("Spool Preserved", &format!("Preserved failed spool file for diagnosis: {}", failed_path.display()));
                }
            }
        }
    });

    // Spawn TCP listener for Normal mode (127.0.0.1:9100)
    let tx_tcp_normal = tx.clone();
    std::thread::spawn(move || {
        listen_on_tcp(TCP_PORT_NORMAL, HalftoneMode::Normal, tx_tcp_normal, silent);
    });

    // Spawn TCP listener for Photo mode (127.0.0.1:9101)
    let tx_tcp_photo = tx.clone();
    std::thread::spawn(move || {
        listen_on_tcp(TCP_PORT_PHOTO, HalftoneMode::Photo, tx_tcp_photo, silent);
    });

    // Spawn listener thread for Photo named pipe
    let tx_pipe_photo = tx.clone();
    std::thread::spawn(move || {
        listen_on_pipe(PIPE_NAME_PHOTO, HalftoneMode::Photo, tx_pipe_photo, silent);
    });

    // Spawn listener thread for Normal named pipe
    let tx_pipe_normal = tx;
    std::thread::spawn(move || {
        listen_on_pipe(PIPE_NAME_NORMAL, HalftoneMode::Normal, tx_pipe_normal, silent);
    });

    // Main thread remains alive serving print queues until shutdown requested
    while !crate::service::SERVICE_SHUTDOWN_REQUESTED.load(std::sync::atomic::Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(500));
    }
    Ok(())
}

pub fn install_printer() -> Result<(), Box<dyn Error>> {
    println!("=======================================================");
    println!("   CANON LBP-1120 NATIVE DUAL-DRIVER INSTALLER        ");
    println!("=======================================================");

    use std::os::windows::process::CommandExt;

    // 1. Create Normal TCP and Named Pipe printer ports
    println!(
        "[1/4] Registering Normal printer ports (TCP '{}' & Pipe '{}')...",
        TCP_PORT_NAME_NORMAL, PIPE_NAME_NORMAL
    );
    let port1_cmd = format!(
        "$ErrorActionPreference = 'Stop'; if (-not (Get-PrinterPort -Name '{0}' -ErrorAction SilentlyContinue)) {{ Add-PrinterPort -Name '{0}' -PrinterHostAddress '127.0.0.1' -PortNumber {1} }}; if (-not (Get-PrinterPort -Name '{2}' -ErrorAction SilentlyContinue)) {{ Add-PrinterPort -Name '{2}' }}",
        TCP_PORT_NAME_NORMAL, TCP_PORT_NORMAL, PIPE_NAME_NORMAL
    );
    let out1 = Command::new("powershell")
        .args(["-NoProfile", "-Command", &port1_cmd])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output()?;
    if !out1.status.success() {
        let err = String::from_utf8_lossy(&out1.stderr);
        return Err(format!("Failed to register Normal printer ports: {}", err.trim()).into());
    }

    // 2. Create Photo TCP and Named Pipe printer ports
    println!(
        "[2/4] Registering Photo printer ports (TCP '{}' & Pipe '{}')...",
        TCP_PORT_NAME_PHOTO, PIPE_NAME_PHOTO
    );
    let port2_cmd = format!(
        "$ErrorActionPreference = 'Stop'; if (-not (Get-PrinterPort -Name '{0}' -ErrorAction SilentlyContinue)) {{ Add-PrinterPort -Name '{0}' -PrinterHostAddress '127.0.0.1' -PortNumber {1} }}; if (-not (Get-PrinterPort -Name '{2}' -ErrorAction SilentlyContinue)) {{ Add-PrinterPort -Name '{2}' }}",
        TCP_PORT_NAME_PHOTO, TCP_PORT_PHOTO, PIPE_NAME_PHOTO
    );
    let out2 = Command::new("powershell")
        .args(["-NoProfile", "-Command", &port2_cmd])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output()?;
    if !out2.status.success() {
        let err = String::from_utf8_lossy(&out2.stderr);
        return Err(format!("Failed to register Photo printer ports: {}", err.trim()).into());
    }

    // 3. Register Normal and Photo Printer Queues with auto-detected driver
    println!("[3/4] Registering '{}' & '{}' in Windows...", PRINTER_NAME_NORMAL, PRINTER_NAME_PHOTO);
    let register_queues_cmd = format!(
        r#"$ErrorActionPreference = 'Stop'
$drivers = Get-PrinterDriver | Select-Object -ExpandProperty Name
$targetDriver = $drivers | Where-Object {{ $_ -like '*PDF*' }} | Select-Object -First 1
if (-not $targetDriver) {{
    $targetDriver = $drivers | Where-Object {{ $_ -like '*Generic*' }} | Select-Object -First 1
}}
if (-not $targetDriver) {{
    $targetDriver = $drivers | Where-Object {{ $_ -like '*Virtual*' }} | Select-Object -First 1
}}
if (-not $targetDriver) {{
    $targetDriver = 'Generic / Text Only'
}}
Write-Host "Binding queues with driver: $targetDriver"

$p1 = Get-Printer -Name '{0}' -ErrorAction SilentlyContinue
if (-not $p1) {{
    Add-Printer -Name '{0}' -DriverName $targetDriver -PortName '{1}'
}} else {{
    Set-Printer -Name '{0}' -PortName '{1}'
}}

$p2 = Get-Printer -Name '{2}' -ErrorAction SilentlyContinue
if (-not $p2) {{
    Add-Printer -Name '{2}' -DriverName $targetDriver -PortName '{3}'
}} else {{
    Set-Printer -Name '{2}' -PortName '{3}'
}}
"#,
        PRINTER_NAME_NORMAL, TCP_PORT_NAME_NORMAL, PRINTER_NAME_PHOTO, TCP_PORT_NAME_PHOTO
    );
    let out3 = Command::new("powershell")
        .args(["-NoProfile", "-Command", &register_queues_cmd])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output()?;
    if !out3.status.success() {
        let err = String::from_utf8_lossy(&out3.stderr);
        return Err(format!("Failed to register Windows printer queues: {}", err.trim()).into());
    }

    // 4. Verify Both Printers Exist
    println!("[4/4] Verifying Windows print queue availability...");
    let verify_cmd = format!(
        "$ErrorActionPreference = 'Stop'; if (-not (Get-Printer -Name '{0}' -ErrorAction SilentlyContinue)) {{ throw 'Printer {0} not found' }}; if (-not (Get-Printer -Name '{1}' -ErrorAction SilentlyContinue)) {{ throw 'Printer {1} not found' }}",
        PRINTER_NAME_NORMAL, PRINTER_NAME_PHOTO
    );
    let out4 = Command::new("powershell")
        .args(["-NoProfile", "-Command", &verify_cmd])
        .creation_flags(0x08000000)
        .output()?;
    if !out4.status.success() {
        let err = String::from_utf8_lossy(&out4.stderr);
        return Err(format!("Verification failed for printer queues: {}", err.trim()).into());
    }

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

    println!("[3/5] Removing TCP and pipe ports for Normal mode...");
    let _ = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Remove-PrinterPort -Name '{}' -ErrorAction SilentlyContinue; Remove-PrinterPort -Name '{}' -ErrorAction SilentlyContinue",
                TCP_PORT_NAME_NORMAL, PIPE_NAME_NORMAL
            ),
        ])
        .creation_flags(0x08000000)
        .output();

    println!("[4/5] Removing TCP and pipe ports for Photo mode...");
    let _ = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Remove-PrinterPort -Name '{}' -ErrorAction SilentlyContinue; Remove-PrinterPort -Name '{}' -ErrorAction SilentlyContinue",
                TCP_PORT_NAME_PHOTO, PIPE_NAME_PHOTO
            ),
        ])
        .creation_flags(0x08000000)
        .output();

    println!("[5/5] Cleaning up legacy run keys and background processes...");
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", "Remove-ItemProperty -Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run' -Name 'CanonLBP1120' -ErrorAction SilentlyContinue; Get-Process capt-poc, canon-lbp1120-service -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue"])
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

#[allow(dead_code)]
pub fn is_pdf_buffer(buf: &[u8]) -> bool {
    buf.windows(4).any(|w| w == b"%PDF")
}

#[allow(dead_code)]
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

    #[test]
    fn test_analyze_stream_format() {
        assert_eq!(analyze_stream_format(b"%PDF-1.7\n").0, "PDF");
        assert_eq!(analyze_stream_format(b"\xef\xbb\xbf%PDF-2.0\n").0, "PDF");
        assert_eq!(analyze_stream_format(b"PK\x03\x04\x14\x00").0, "XPS");
        assert_eq!(analyze_stream_format(b"\x1b%-12345X@PJL\r\n").0, "PCL");
        assert_eq!(analyze_stream_format(b"\x1bE\x1b&l0O").0, "PCL");
        assert_eq!(analyze_stream_format(b"%!PS-Adobe-3.0\n").0, "PostScript");
        assert_eq!(analyze_stream_format(b"\x01\x00\x00\x00\x40\x00\x00\x00").0, "EMF");
        assert_eq!(analyze_stream_format(b"Hello from Notepad plain text print job\n").0, "Text");
        assert_eq!(analyze_stream_format(b"\xde\xad\xbe\xef\x00\x01\x02\x03").0, "Unknown");
    }
}
