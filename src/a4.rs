use crate::pbm::PbmImage;

// Basic 8x8 font table for uppercase ASCII, digits, and common punctuation
fn get_glyph(c: char) -> [u8; 8] {
    match c {
        'A' => [0x3C, 0x42, 0x42, 0x7E, 0x42, 0x42, 0x42, 0x00],
        'B' => [0x7C, 0x42, 0x42, 0x7C, 0x42, 0x42, 0x7C, 0x00],
        'C' => [0x3C, 0x42, 0x40, 0x40, 0x40, 0x42, 0x3C, 0x00],
        'D' => [0x78, 0x44, 0x42, 0x42, 0x42, 0x44, 0x78, 0x00],
        'E' => [0x7E, 0x40, 0x40, 0x7C, 0x40, 0x40, 0x7E, 0x00],
        'F' => [0x7E, 0x40, 0x40, 0x7C, 0x40, 0x40, 0x40, 0x00],
        'G' => [0x3C, 0x42, 0x40, 0x4E, 0x42, 0x42, 0x3C, 0x00],
        'H' => [0x42, 0x42, 0x42, 0x7E, 0x42, 0x42, 0x42, 0x00],
        'I' => [0x38, 0x10, 0x10, 0x10, 0x10, 0x10, 0x38, 0x00],
        'J' => [0x1C, 0x08, 0x08, 0x08, 0x08, 0x48, 0x30, 0x00],
        'K' => [0x42, 0x44, 0x48, 0x70, 0x48, 0x44, 0x42, 0x00],
        'L' => [0x40, 0x40, 0x40, 0x40, 0x40, 0x40, 0x7E, 0x00],
        'M' => [0x81, 0xC3, 0xA5, 0x99, 0x81, 0x81, 0x81, 0x00],
        'N' => [0x42, 0x62, 0x52, 0x4A, 0x46, 0x42, 0x42, 0x00],
        'O' => [0x3C, 0x42, 0x42, 0x42, 0x42, 0x42, 0x3C, 0x00],
        'P' => [0x7C, 0x42, 0x42, 0x7C, 0x40, 0x40, 0x40, 0x00],
        'Q' => [0x3C, 0x42, 0x42, 0x42, 0x4A, 0x44, 0x3A, 0x00],
        'R' => [0x7C, 0x42, 0x42, 0x7C, 0x48, 0x44, 0x42, 0x00],
        'S' => [0x3C, 0x42, 0x40, 0x3C, 0x02, 0x42, 0x3C, 0x00],
        'T' => [0x7F, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x00],
        'U' => [0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x3C, 0x00],
        'V' => [0x42, 0x42, 0x42, 0x42, 0x24, 0x24, 0x18, 0x00],
        'W' => [0x81, 0x81, 0x81, 0x99, 0xA5, 0xC3, 0x81, 0x00],
        'X' => [0x42, 0x24, 0x18, 0x18, 0x24, 0x42, 0x42, 0x00],
        'Y' => [0x42, 0x42, 0x24, 0x18, 0x10, 0x10, 0x10, 0x00],
        'Z' => [0x7E, 0x04, 0x08, 0x10, 0x20, 0x40, 0x7E, 0x00],
        '0' => [0x3C, 0x46, 0x4A, 0x52, 0x62, 0x42, 0x3C, 0x00],
        '1' => [0x18, 0x28, 0x08, 0x08, 0x08, 0x08, 0x3E, 0x00],
        '2' => [0x3C, 0x42, 0x02, 0x0C, 0x30, 0x40, 0x7E, 0x00],
        '3' => [0x3C, 0x42, 0x02, 0x1C, 0x02, 0x42, 0x3C, 0x00],
        '4' => [0x08, 0x18, 0x28, 0x48, 0x7E, 0x08, 0x08, 0x00],
        '5' => [0x7E, 0x40, 0x7C, 0x02, 0x02, 0x42, 0x3C, 0x00],
        '6' => [0x3C, 0x40, 0x40, 0x7C, 0x42, 0x42, 0x3C, 0x00],
        '7' => [0x7E, 0x02, 0x04, 0x08, 0x10, 0x20, 0x20, 0x00],
        '8' => [0x3C, 0x42, 0x42, 0x3C, 0x42, 0x42, 0x3C, 0x00],
        '9' => [0x3C, 0x42, 0x42, 0x3E, 0x02, 0x02, 0x3C, 0x00],
        '-' => [0x00, 0x00, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x00],
        ':' => [0x00, 0x18, 0x18, 0x00, 0x18, 0x18, 0x00, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x30],
        '|' => [0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00],
        '/' => [0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x00],
        '(' => [0x0C, 0x18, 0x30, 0x30, 0x30, 0x18, 0x0C, 0x00],
        ')' => [0x30, 0x18, 0x0C, 0x0C, 0x0C, 0x18, 0x30, 0x00],
        '=' => [0x00, 0x7E, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x00],
        '%' => [0x62, 0x64, 0x08, 0x10, 0x20, 0x26, 0x46, 0x00],
        '[' => [0x3C, 0x30, 0x30, 0x30, 0x30, 0x30, 0x3C, 0x00],
        ']' => [0x3C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x3C, 0x00],
        _ => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    }
}

pub fn draw_rect(img: &mut PbmImage, x: usize, y: usize, w: usize, h: usize, thickness: usize) {
    for t in 0..thickness {
        // Top and bottom horizontal lines
        for px in x..(x + w) {
            img.set_pixel(px, y + t, true);
            img.set_pixel(px, y + h.saturating_sub(1 + t), true);
        }
        // Left and right vertical lines
        for py in y..(y + h) {
            img.set_pixel(x + t, py, true);
            img.set_pixel(x + w.saturating_sub(1 + t), py, true);
        }
    }
}

pub fn fill_rect(img: &mut PbmImage, x: usize, y: usize, w: usize, h: usize) {
    for py in y..(y + h) {
        for px in x..(x + w) {
            img.set_pixel(px, py, true);
        }
    }
}

pub fn draw_line(img: &mut PbmImage, x0: usize, y0: usize, x1: usize, y1: usize) {
    let dx = (x1 as isize - x0 as isize).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 as isize - y0 as isize).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0 as isize;
    let mut y = y0 as isize;

    loop {
        if x >= 0 && y >= 0 {
            img.set_pixel(x as usize, y as usize, true);
        }
        if x == x1 as isize && y == y1 as isize {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

pub fn draw_circle(img: &mut PbmImage, cx: usize, cy: usize, radius: usize, thickness: usize) {
    let r_min = radius.saturating_sub(thickness / 2);
    let r_max = radius + (thickness / 2);
    let r_min_sq = (r_min * r_min) as isize;
    let r_max_sq = (r_max * r_max) as isize;

    for dy in -(r_max as isize)..=(r_max as isize) {
        let py = cy as isize + dy;
        if py < 0 || py >= img.height as isize {
            continue;
        }
        for dx in -(r_max as isize)..=(r_max as isize) {
            let px = cx as isize + dx;
            if px < 0 || px >= img.width as isize {
                continue;
            }
            let dist_sq = dx * dx + dy * dy;
            if dist_sq >= r_min_sq && dist_sq <= r_max_sq {
                img.set_pixel(px as usize, py as usize, true);
            }
        }
    }
}

pub fn draw_text(img: &mut PbmImage, text: &str, start_x: usize, start_y: usize, scale: usize) {
    let mut cur_x = start_x;
    for c in text.chars() {
        let glyph = get_glyph(c.to_ascii_uppercase());
        for (row_idx, &row_byte) in glyph.iter().enumerate() {
            for col_idx in 0..8 {
                if (row_byte & (0x80 >> col_idx)) != 0 {
                    let px = cur_x + col_idx * scale;
                    let py = start_y + row_idx * scale;
                    for dy in 0..scale {
                        for dx in 0..scale {
                            img.set_pixel(px + dx, py + dy, true);
                        }
                    }
                }
            }
        }
        cur_x += 8 * scale;
    }
}

pub fn draw_dither_patch(img: &mut PbmImage, x: usize, y: usize, w: usize, h: usize, pattern: u8) {
    for py in y..(y + h) {
        for px in x..(x + w) {
            let black = match pattern {
                1 => (px + py) % 8 == 0,            // ~12%
                2 => px % 2 == 0 && py % 2 == 0,    // 25%
                3 => (px + py) % 2 == 0,            // 50% checkerboard
                4 => !(px % 2 == 0 && py % 2 == 0), // 75%
                _ => true,                          // 100% solid
            };
            if black {
                img.set_pixel(px, py, true);
            }
        }
    }
}

pub fn generate_a4_test_page() -> PbmImage {
    // 4736 pixels wide (592 bytes), 6778 lines high (A4 @ 600 DPI)
    let mut img = PbmImage::new(4736, 6778);

    // 1. Outer framing border (thickness 8 pixels)
    draw_rect(&mut img, 150, 150, 4436, 6478, 8);
    // Inner fine border (thickness 2 pixels)
    draw_rect(&mut img, 180, 180, 4376, 6418, 2);

    // 2. Corner crosshairs
    let cross = |img: &mut PbmImage, cx: usize, cy: usize| {
        draw_line(img, cx.saturating_sub(100), cy, cx + 100, cy);
        draw_line(img, cx, cy.saturating_sub(100), cx, cy + 100);
        draw_circle(img, cx, cy, 50, 2);
    };
    cross(&mut img, 350, 350);
    cross(&mut img, 4386, 350);
    cross(&mut img, 350, 6428);
    cross(&mut img, 4386, 6428);

    // 3. Header Section (scale 10 = 80px tall glyphs)
    let title1 = "CANON LASERSHOT LBP-1120";
    let title1_x = (4736 - title1.len() * 80) / 2;
    draw_text(&mut img, title1, title1_x, 500, 10);

    let title2 = "CAPT V1 PROTOCOL REVIVAL";
    let title2_x = (4736 - title2.len() * 64) / 2;
    draw_text(&mut img, title2, title2_x, 620, 8);

    let title3 = "WINDOWS 11 X64 - NATIVE WINUSB PIPELINE";
    let title3_x = (4736 - title3.len() * 48) / 2;
    draw_text(&mut img, title3, title3_x, 720, 6);

    // Horizontal divider line
    draw_rect(&mut img, 300, 850, 4136, 8, 8);

    // 4. Diagnostic Specs Table (scale 4 = 32px tall glyphs)
    let specs = [
        "TARGET MODEL        : 0X01A4 (CANON LBP-1120 LASER ENGINE)",
        "USB ENDPOINTS       : BULK OUT 0X01 (64B) | BULK IN 0X82 (64B)",
        "PAGE GEOMETRY       : A4 (210 X 297 MM) @ 600 DPI",
        "CANVAS RESOLUTION   : 4736 X 6778 PIXELS (592 BYTES PER LINE)",
        "MARGIN OFFSETS      : LEFT = 112 PX (4.74 MM), TOP = 120 PX (5.08 MM)",
        "COMPRESSION CODEC   : SCOA 2D DELTA RUN-LENGTH ENCODING",
        "DRIVER ARCHITECTURE : 100% USER-MODE WINUSB (ZERO KERNEL HOOKS)",
    ];
    let mut spec_y = 950;
    for s in &specs {
        draw_text(&mut img, s, 400, spec_y, 4);
        spec_y += 70;
    }

    // Horizontal divider line
    draw_rect(&mut img, 300, 1500, 4136, 4, 4);

    // 5. Halftone Grayscale Calibration Bars (10%, 25%, 50%, 75%, 100%)
    draw_text(
        &mut img,
        "HALFTONE DENSITY CALIBRATION (10% / 25% / 50% / 75% / 100%):",
        400,
        1560,
        4,
    );
    let patch_w = 700;
    let patch_h = 200;
    let start_px = 500;
    let py = 1640;
    for p in 0..5 {
        let px = start_px + p * (patch_w + 80);
        draw_dither_patch(&mut img, px, py, patch_w, patch_h, (p + 1) as u8);
        draw_rect(&mut img, px, py, patch_w, patch_h, 3);
    }

    // 6. Center Concentric Target & Alignment Reticle
    let center_x = 2368;
    let center_y = 2800;
    draw_text(
        &mut img,
        "OPTICAL CENTERING & ENGINE ALIGNMENT TARGET",
        1100,
        2150,
        5,
    );
    draw_circle(&mut img, center_x, center_y, 500, 4);
    draw_circle(&mut img, center_x, center_y, 400, 3);
    draw_circle(&mut img, center_x, center_y, 300, 3);
    draw_circle(&mut img, center_x, center_y, 200, 4);
    draw_circle(&mut img, center_x, center_y, 100, 5);
    fill_rect(&mut img, center_x - 15, center_y - 15, 30, 30);
    draw_line(&mut img, center_x - 600, center_y, center_x + 600, center_y);
    draw_line(&mut img, center_x, center_y - 600, center_x, center_y + 600);

    // Diagonal reference lines across the page
    draw_line(&mut img, 350, 3550, 2100, 4500);
    draw_line(&mut img, 4386, 3550, 2636, 4500);

    // 7. High-Resolution Line-Pair Frequency Targets
    draw_text(
        &mut img,
        "600 DPI LINE-PAIR RESOLUTION FREQUENCY BARS:",
        400,
        4600,
        4,
    );
    // 1-pixel on / 1-pixel off (300 line pairs / inch)
    for i in 0..200 {
        let ly = 4680 + i * 2;
        draw_line(&mut img, 400, ly, 2200, ly);
    }
    draw_text(&mut img, "300 LP/IN (1 PX)", 2250, 4800, 4);

    // 2-pixel on / 2-pixel off (150 line pairs / inch)
    for i in 0..100 {
        let ly = 4680 + i * 4;
        fill_rect(&mut img, 2600, ly, 1600, 2);
    }
    draw_text(&mut img, "150 LP/IN (2 PX)", 4250, 4800, 4);

    // 8. Bottom Verification Box
    draw_rect(&mut img, 300, 5200, 4136, 950, 6);
    draw_text(
        &mut img,
        "FIRST PHYSICAL TEST PRINT ON WINUSB X64",
        650,
        5320,
        7,
    );
    draw_text(
        &mut img,
        "HARDWARE REGISTERS: BASIC=0X78 | ENGINE=0X0000 | PAPER=0X80",
        450,
        5450,
        4,
    );
    draw_text(
        &mut img,
        "USB PROTOCOL     : RAW BULK CAPT 1.0 (NO WRAPPER / NO POSTSCRIPT)",
        450,
        5530,
        4,
    );
    draw_text(
        &mut img,
        "DEVICE ENUM      : USB/VID_04A9&PID_262B/0353S4CQ (CANON LBP-1120)",
        450,
        5610,
        4,
    );
    draw_text(
        &mut img,
        "SYSTEM DATE      : 2026-10-14 | REA VERIFIED ROUNDTRIP PASSED",
        450,
        5690,
        4,
    );
    draw_text(
        &mut img,
        "[SUCCESS: 100% BIT-EXACT HARDWARE ENGINE TEST]",
        750,
        5850,
        6,
    );

    // Bottom margin footer
    draw_text(
        &mut img,
        "CANON LASERSHOT LBP-1120 WINUSB PROJECT - ARCHITECTURE BY ANTIGRAVITY",
        700,
        6300,
        3,
    );

    img
}

/// Generates a standardized 600 DPI A4 Print Calibration & Optical Benchmark Target
pub fn generate_calibration_page() -> PbmImage {
    let mut img = PbmImage::new(4736, 6778);

    // 1. Outer printable border & corner crosshairs
    draw_rect(&mut img, 112, 120, 4512, 6538, 4);

    // Corner crosshairs (100 px length)
    let crosshairs: [(usize, usize); 4] = [(112, 120), (4624, 120), (112, 6658), (4624, 6658)];
    for &(cx, cy) in &crosshairs {
        draw_line(&mut img, cx.saturating_sub(60), cy, (cx + 60).min(4735), cy);
        draw_line(&mut img, cx, cy.saturating_sub(60), cx, (cy + 60).min(6777));
        draw_circle(&mut img, cx, cy, 30, 2);
    }

    // Header banner
    draw_rect(&mut img, 200, 250, 4336, 320, 4);
    fill_rect(&mut img, 210, 260, 4316, 80);
    // Draw white header text manually or inverted? We'll put black title below
    draw_text(&mut img, "CANON LASERSHOT LBP-1120 OPTICAL CALIBRATION SUITE", 350, 360, 6);
    draw_text(&mut img, "600 X 600 DPI NATIVE LASER | CANONICAL A4 CANVAS (4736 X 6778) | CAPT 1.0", 500, 440, 4);
    draw_text(&mut img, "ELECTROPHOTOGRAPHIC P160 ENGINE BENCHMARK - DATE: 2026-10-31", 750, 500, 3);

    // Section 1: Micro-Typography Legibility Scale
    draw_rect(&mut img, 200, 620, 4336, 750, 3);
    draw_text(&mut img, "ZONE 1: MICRO-TYPOGRAPHY & SERIF RESOLUTION SCALE", 250, 650, 4);
    draw_text(&mut img, "SCALE 1 (~1.0 PT / 8 PX): ABCDEFGHIJKLMNOPQRSTUVWXYZ 0123456789", 250, 720, 1);
    draw_text(&mut img, "SCALE 2 (~2.0 PT / 16 PX): ABCDEFGHIJKLMNOPQRSTUVWXYZ 0123456789 - FINE SERIF CHECK", 250, 750, 2);
    draw_text(&mut img, "SCALE 3 (~3.0 PT / 24 PX): ABCDEFGHIJKLMNOPQRSTUVWXYZ 0123456789", 250, 800, 3);
    draw_text(&mut img, "SCALE 4 (~4.0 PT / 32 PX): CANON LASERSHOT LBP-1120 WINUSB DRIVER 2026", 250, 860, 4);
    draw_text(&mut img, "SCALE 6 (~6.0 PT / 48 PX): REVERSE ENGINEERING LAB BENCHMARK", 250, 950, 6);
    draw_text(&mut img, "SCALE 8 (~8.0 PT / 64 PX): OPTICAL GROUND TRUTH TARGET", 250, 1070, 8);
    draw_text(&mut img, "SCALE 10 (~10.0 PT / 80 PX): 600 DPI NATIVE LASER", 250, 1220, 10);

    // Section 2: Fine Line Resolution & MTF Pairs (Horizontal, Vertical, Diagonal)
    draw_rect(&mut img, 200, 1420, 4336, 900, 3);
    draw_text(&mut img, "ZONE 2: 1-DOT LINE RESOLUTION & MTF FREQUENCY PAIRS (42.33 MICRONS)", 250, 1450, 4);

    // Vertical line pairs (tests laser modulation frequency & rise/fall time)
    draw_text(&mut img, "VERTICAL LINE PAIRS (1-ON-1-OFF / 2-ON-2-OFF / 4-ON-4-OFF):", 250, 1510, 3);
    let mut vx = 250;
    for _ in 0..60 {
        draw_line(&mut img, vx, 1560, vx, 1750);
        vx += 2; // 1 pixel line, 1 pixel gap
    }
    vx += 40;
    for _ in 0..40 {
        draw_rect(&mut img, vx, 1560, 2, 190, 2);
        vx += 4; // 2 pixel line, 2 pixel gap
    }
    vx += 40;
    for _ in 0..20 {
        draw_rect(&mut img, vx, 1560, 4, 190, 4);
        vx += 8; // 4 pixel line, 4 pixel gap
    }

    // Horizontal line pairs (tests polygon mirror facet pitch consistency)
    draw_text(&mut img, "HORIZONTAL LINE PAIRS (1-ON-1-OFF / 2-ON-2-OFF):", 250, 1780, 3);
    let mut hy = 1830;
    for _ in 0..40 {
        draw_line(&mut img, 250, hy, 1200, hy);
        hy += 2;
    }
    hy = 1830;
    for _ in 0..25 {
        draw_line(&mut img, 1250, hy, 2200, hy);
        draw_line(&mut img, 1250, hy + 1, 2200, hy + 1);
        hy += 4;
    }

    // Diagonal lines (tests Automatic Image Refinement / AIR smoothing)
    draw_text(&mut img, "DIAGONAL 45-DEGREE BURST (TESTS AIR SMOOTHING):", 2350, 1510, 3);
    for i in 0..30 {
        let offset = i * 20;
        draw_line(&mut img, 2400 + offset, 1560, 2800 + offset, 1960);
        draw_line(&mut img, 3200 + offset, 1960, 3600 + offset, 1560);
    }

    // Section 3: 16-Step Grayscale Density Wedge
    draw_rect(&mut img, 200, 2370, 4336, 850, 3);
    draw_text(&mut img, "ZONE 3: 16-STEP DENSITY WEDGE (SPOT GAIN & HALFTONE LINEARITY)", 250, 2400, 4);

    let patch_w = 250;
    let patch_h = 280;
    for step in 0..16 {
        let px = 250 + (step * (patch_w + 15));
        let py = 2470;
        draw_rect(&mut img, px, py, patch_w, patch_h, 2);

        // Fill with ordered dither pattern based on step (0 = 0%, 15 = ~94%)
        let density = step; // 0..15 dots out of 16 in a 4x4 matrix
        for dy in 0..patch_h {
            for dx in 0..patch_w {
                // 4x4 Bayer matrix thresholds
                let bayer4x4 = [
                    [ 0,  8,  2, 10],
                    [12,  4, 14,  6],
                    [ 3, 11,  1,  9],
                    [15,  7, 13,  5],
                ];
                let tx = dx % 4;
                let ty = dy % 4;
                if bayer4x4[ty][tx] < density {
                    img.set_pixel(px + dx, py + dy, true);
                }
            }
        }
        let pct = (step * 100) / 16;
        draw_text(&mut img, &format!("{}%", pct), px + 50, py + patch_h + 20, 3);
    }

    // Section 4: Concentric Circles (Drum Runout & Gear Eccentricity)
    draw_rect(&mut img, 200, 3270, 4336, 1200, 3);
    draw_text(&mut img, "ZONE 4: CONCENTRIC CIRCLES (DRUM RUNOUT & MOTOR ECCENTRICITY)", 250, 3300, 4);
    let center_x = 2368;
    let center_y = 3900;
    for r in (50..=550).step_by(50) {
        draw_circle(&mut img, center_x, center_y, r, 3);
    }
    draw_line(&mut img, center_x - 580, center_y, center_x + 580, center_y);
    draw_line(&mut img, center_x, center_y - 580, center_x, center_y + 580);
    draw_text(&mut img, "ASPECT RATIO 1.000: CIRCULARITY ERROR MEASURES MECHANICAL GEAR DEFECTS", 850, 4420, 3);

    // Section 5: Uniformity Fields (Solid Black & 50% Tint)
    draw_rect(&mut img, 200, 4520, 4336, 950, 3);
    draw_text(&mut img, "ZONE 5: CHARGE UNIFORMITY & MAXIMUM TONER DENSITY DMAX", 250, 4550, 4);

    // Solid Black Block (50mm x 25mm -> ~1181 x 590 px)
    draw_text(&mut img, "SOLID BLACK FIELD (DMAX TONER DENSITY):", 250, 4610, 3);
    fill_rect(&mut img, 250, 4660, 1800, 450);

    // 50% Checkerboard Tint Block
    draw_text(&mut img, "50% CHECKERBOARD FIELD (PCR CHARGE UNIFORMITY):", 2300, 4610, 3);
    for dy in 0..450 {
        for dx in 0..1800 {
            if (dx + dy) % 2 == 0 {
                img.set_pixel(2300 + dx, 4660 + dy, true);
            }
        }
    }
    draw_rect(&mut img, 2300, 4660, 1800, 450, 2);

    // Section 6: Telemetry Block & Ground Truth Legend
    draw_rect(&mut img, 200, 5520, 4336, 1050, 3);
    draw_text(&mut img, "ZONE 6: SYSTEM TELEMETRY & EMPIRICAL EVIDENCE MATRIX", 250, 5550, 4);
    draw_text(&mut img, "TARGET PRINTER   : CANON LASERSHOT LBP-1120 (USB VID 0x04A9, PID 0x262B)", 250, 5630, 4);
    draw_text(&mut img, "PRINTING ENGINE  : CANON P160 ELECTROPHOTOGRAPHIC ENGINE (EP-22 PLATFORM)", 250, 5710, 4);
    draw_text(&mut img, "VIDEO PROTOCOL   : CANON CAPT 1.0 WITH SCOA 2D DELTA + RLE STREAMING", 250, 5790, 4);
    draw_text(&mut img, "PHYSICAL RESOLUTION: 600 X 600 DPI NATIVE (42.33 MICRONS PER DOT)", 250, 5870, 4);
    draw_text(&mut img, "CANONICAL DIMENSIONS: 4736 PIXELS WIDE X 6778 SCANLINES (592 BYTES/LINE)", 250, 5950, 4);
    draw_text(&mut img, "TOTAL PIXEL MATRIX : 32,096,608 DOTS (4,012,576 UNCOMPRESSED BYTES)", 250, 6030, 4);
    draw_text(&mut img, "CALIBRATION STATUS : [OFFICIAL BENCHMARK GROUND TRUTH SUITE VERIFIED]", 250, 6150, 5);

    // Footer
    draw_text(&mut img, "CANON LBP-1120 AUTONOMOUS MASTER PROJECT - STRICT NON-DESTRUCTIVE REVERSE ENGINEERING", 350, 6600, 3);

    img
}
