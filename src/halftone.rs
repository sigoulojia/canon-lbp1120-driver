#![allow(dead_code)]
//! Advanced Monochrome Image Processing & Halftoning Engine for Canon LBP-1120 (600 DPI)
//!
//! Implements 10 halftoning & thresholding algorithms, pre-processing filters (Gamma, Contrast,
//! 3x3 Unsharp Mask), 6 intelligent document presets, and binary parser for Canon's factory
//! `Cap3ht.bin` dither matrices.

use crate::pbm::PbmImage;
use std::time::Instant;

pub static CANON_CAP3HT_BIN: &[u8] = include_bytes!("../assets/Cap3ht.bin");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DitherAlgorithm {
    NormalClustered8x8,
    CanonFactory16x16,
    FloydSteinberg,
    JarvisJudiceNinke,
    Stucki,
    Atkinson,
    OrderedBayer8x8,
    OtsuGlobal,
    SauvolaAdaptive,
    TonerSaveDraft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintPreset {
    Text,
    MixedDocument,
    Graphics,
    Photograph,
    HighQualityPhotograph,
    DraftTonerSave,
}

#[derive(Debug, Clone, Copy)]
pub struct ImageProcessingConfig {
    pub algorithm: DitherAlgorithm,
    pub gamma: f32,      // 0.5 .. 2.5 (1.0 = neutral)
    pub contrast: f32,   // -50.0 .. +50.0 (0.0 = neutral)
    pub sharpen: f32,    // 0.0 .. 2.0 (0.0 = off)
    pub toner_save: bool,
}

impl ImageProcessingConfig {
    pub fn from_preset(preset: PrintPreset) -> Self {
        match preset {
            PrintPreset::Text => Self {
                algorithm: DitherAlgorithm::OtsuGlobal,
                gamma: 0.90,
                contrast: 22.0,
                sharpen: 0.8,
                toner_save: false,
            },
            PrintPreset::MixedDocument => Self {
                algorithm: DitherAlgorithm::NormalClustered8x8,
                gamma: 1.0,
                contrast: 10.0,
                sharpen: 0.4,
                toner_save: false,
            },
            PrintPreset::Graphics => Self {
                algorithm: DitherAlgorithm::CanonFactory16x16,
                gamma: 1.05,
                contrast: 8.0,
                sharpen: 0.3,
                toner_save: false,
            },
            PrintPreset::Photograph => Self {
                algorithm: DitherAlgorithm::FloydSteinberg,
                gamma: 1.15,
                contrast: 6.0,
                sharpen: 0.25,
                toner_save: false,
            },
            PrintPreset::HighQualityPhotograph => Self {
                algorithm: DitherAlgorithm::Stucki,
                gamma: 1.22,
                contrast: 10.0,
                sharpen: 0.45,
                toner_save: false,
            },
            PrintPreset::DraftTonerSave => Self {
                algorithm: DitherAlgorithm::TonerSaveDraft,
                gamma: 1.35,
                contrast: 0.0,
                sharpen: 0.2,
                toner_save: true,
            },
        }
    }
}

pub const BAYER_8X8: [[u8; 8]; 8] = [
    [  0, 128,  32, 160,   8, 136,  40, 168],
    [192,  64, 224,  96, 200,  72, 232, 104],
    [ 48, 176,  16, 144,  56, 184,  24, 152],
    [240, 112, 208,  80, 248, 120, 216,  88],
    [ 12, 140,  44, 172,   4, 132,  36, 164],
    [204,  76, 236, 108, 196,  68, 228, 100],
    [ 60, 188,  28, 156,  52, 180,  20, 148],
    [252, 124, 220,  92, 244, 116, 212,  84],
];

pub const CLUSTERED_8X8: [[u8; 8]; 8] = [
    [230, 203, 175, 148, 152, 179, 207, 234],
    [226, 144, 115,  87,  91, 119, 148, 238],
    [199, 111,  58,  30,  34,  62, 123, 211],
    [171,  83,  26,   3,   7,  38,  95, 183],
    [167,  79,  22,   0,  11,  42,  99, 187],
    [195, 107,  54,  18,  14,  46, 127, 215],
    [222, 139,  75,  71,  50,  67, 135, 242],
    [246, 191, 163, 131, 103, 159, 175, 250],
];

/// Extracts the authentic 16x16 factory gradation dither matrix (Matrix 1 at offset 0x16030) from `Cap3ht.bin`.
pub fn load_canon_factory_16x16() -> [[u8; 16]; 16] {
    let mut mat = [[128u8; 16]; 16];
    let start = 90084 + 4 + (4 + 64) + 4; // 0x16030
    if CANON_CAP3HT_BIN.len() >= start + 256 {
        for r in 0..16 {
            for c in 0..16 {
                mat[r][c] = CANON_CAP3HT_BIN[start + r * 16 + c];
            }
        }
    }
    mat
}

#[derive(Debug, Clone)]
pub struct HalftoneBenchmarkResult {
    pub algorithm: DitherAlgorithm,
    pub processing_ms: u128,
    pub black_pixels: usize,
    pub coverage_percent: f64,
}

/// Computes optimal global threshold using Otsu's between-class variance maximization.
pub fn compute_otsu_threshold(gray: &[u8]) -> u8 {
    let mut hist = [0u64; 256];
    for &px in gray {
        hist[px as usize] += 1;
    }
    let total = gray.len() as f64;
    if total == 0.0 {
        return 128;
    }
    let mut sum = 0.0;
    for (i, &h) in hist.iter().enumerate() {
        sum += (i as f64) * (h as f64);
    }
    let mut sum_b = 0.0;
    let mut w_b = 0.0;
    let mut max_var = -1.0;
    let mut best_thresh = 128u8;

    for (t, &h) in hist.iter().enumerate() {
        w_b += h as f64;
        if w_b == 0.0 {
            continue;
        }
        let w_f = total - w_b;
        if w_f == 0.0 {
            break;
        }
        sum_b += (t as f64) * (h as f64);
        let m_b = sum_b / w_b;
        let m_f = (sum - sum_b) / w_f;
        let var_between = w_b * w_f * (m_b - m_f) * (m_b - m_f);
        if var_between > max_var {
            max_var = var_between;
            best_thresh = t as u8;
        }
    }
    best_thresh
}

/// Applies gamma, contrast, and optional 3x3 unsharp mask to an 8-bit grayscale buffer.
pub fn preprocess_grayscale(
    src: &[u8],
    width: usize,
    height: usize,
    cfg: &ImageProcessingConfig,
) -> Vec<u8> {
    let mut lut = [0u8; 256];
    let inv_gamma = 1.0 / cfg.gamma.clamp(0.2, 3.0);
    let factor = (259.0 * (cfg.contrast + 255.0)) / (255.0 * (259.0 - cfg.contrast));
    for (i, item) in lut.iter_mut().enumerate() {
        let norm = (i as f32) / 255.0;
        let g = norm.powf(inv_gamma) * 255.0;
        let c = factor * (g - 128.0) + 128.0;
        *item = c.round().clamp(0.0, 255.0) as u8;
    }

    let mapped: Vec<u8> = src.iter().map(|&p| lut[p as usize]).collect();
    if cfg.sharpen <= 0.01 || width < 3 || height < 3 {
        return mapped;
    }

    let mut out = mapped.clone();
    let strength = cfg.sharpen.clamp(0.0, 2.0);
    for y in 1..(height - 1) {
        let row = y * width;
        for x in 1..(width - 1) {
            let idx = row + x;
            let center = mapped[idx] as f32;
            let neighbors = (mapped[idx - 1] as f32
                + mapped[idx + 1] as f32
                + mapped[idx - width] as f32
                + mapped[idx + width] as f32)
                * 0.25;
            let val = center + strength * (center - neighbors);
            out[idx] = val.round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// Converts an 8-bit grayscale image (`width * height`) into a 1-bit `PbmImage` using the specified configuration.
pub fn process_grayscale_to_pbm(
    gray_input: &[u8],
    width: usize,
    height: usize,
    cfg: &ImageProcessingConfig,
) -> (PbmImage, HalftoneBenchmarkResult) {
    let t0 = Instant::now();
    let gray = preprocess_grayscale(gray_input, width, height, cfg);
    let mut pbm = PbmImage::new(width, height);
    let mut black_pixels = 0usize;

    match cfg.algorithm {
        DitherAlgorithm::NormalClustered8x8 => {
            for y in 0..height {
                let line = pbm.line_mut(y);
                let row = y * width;
                for x in 0..width {
                    let g = gray[row + x];
                    let is_black = if g <= 55 {
                        true
                    } else if g >= 225 {
                        false
                    } else {
                        g < CLUSTERED_8X8[y % 8][x % 8]
                    };
                    if is_black && (!cfg.toner_save || ((x + y) % 3 != 0)) {
                        line[x / 8] |= 1 << (7 - (x % 8));
                        black_pixels += 1;
                    }
                }
            }
        }
        DitherAlgorithm::CanonFactory16x16 => {
            let mat = load_canon_factory_16x16();
            for y in 0..height {
                let line = pbm.line_mut(y);
                let row = y * width;
                for x in 0..width {
                    let g = gray[row + x];
                    let is_black = g < mat[y % 16][x % 16];
                    if is_black {
                        line[x / 8] |= 1 << (7 - (x % 8));
                        black_pixels += 1;
                    }
                }
            }
        }
        DitherAlgorithm::OrderedBayer8x8 => {
            for y in 0..height {
                let line = pbm.line_mut(y);
                let row = y * width;
                for x in 0..width {
                    if gray[row + x] < BAYER_8X8[y % 8][x % 8] {
                        line[x / 8] |= 1 << (7 - (x % 8));
                        black_pixels += 1;
                    }
                }
            }
        }
        DitherAlgorithm::OtsuGlobal => {
            let thresh = compute_otsu_threshold(&gray);
            for y in 0..height {
                let line = pbm.line_mut(y);
                let row = y * width;
                for x in 0..width {
                    if gray[row + x] <= thresh {
                        line[x / 8] |= 1 << (7 - (x % 8));
                        black_pixels += 1;
                    }
                }
            }
        }
        DitherAlgorithm::TonerSaveDraft => {
            for y in 0..height {
                let line = pbm.line_mut(y);
                let row = y * width;
                for x in 0..width {
                    let g = gray[row + x];
                    // Economy toner gate: hollow interior of solid regions while keeping edges crisp
                    let is_black = g < 140 && ((x + y) % 2 == 0 || g < 40 && (x % 3 == 0));
                    if is_black {
                        line[x / 8] |= 1 << (7 - (x % 8));
                        black_pixels += 1;
                    }
                }
            }
        }
        DitherAlgorithm::SauvolaAdaptive => {
            // Integral image accelerated Sauvola thresholding (window 15x15, k = 0.2)
            let w1 = width + 1;
            let mut int_sum = vec![0u64; w1 * (height + 1)];
            let mut int_sq = vec![0u64; w1 * (height + 1)];
            for y in 0..height {
                let mut r_sum = 0u64;
                let mut r_sq = 0u64;
                for x in 0..width {
                    let v = gray[y * width + x] as u64;
                    r_sum += v;
                    r_sq += v * v;
                    int_sum[(y + 1) * w1 + (x + 1)] = int_sum[y * w1 + (x + 1)] + r_sum;
                    int_sq[(y + 1) * w1 + (x + 1)] = int_sq[y * w1 + (x + 1)] + r_sq;
                }
            }
            let radius = 7usize;
            for y in 0..height {
                let y0 = y.saturating_sub(radius);
                let y1 = (y + radius + 1).min(height);
                let line = pbm.line_mut(y);
                for x in 0..width {
                    let x0 = x.saturating_sub(radius);
                    let x1 = (x + radius + 1).min(width);
                    let area = ((y1 - y0) * (x1 - x0)) as f64;
                    let sum = (int_sum[y1 * w1 + x1] + int_sum[y0 * w1 + x0]
                        - int_sum[y0 * w1 + x1]
                        - int_sum[y1 * w1 + x0]) as f64;
                    let sq = (int_sq[y1 * w1 + x1] + int_sq[y0 * w1 + x0]
                        - int_sq[y0 * w1 + x1]
                        - int_sq[y1 * w1 + x0]) as f64;
                    let mean = sum / area;
                    let variance = (sq / area - mean * mean).max(0.0);
                    let std_dev = variance.sqrt();
                    let threshold = mean * (1.0 + 0.20 * ((std_dev / 128.0) - 1.0));
                    if (gray[y * width + x] as f64) <= threshold {
                        line[x / 8] |= 1 << (7 - (x % 8));
                        black_pixels += 1;
                    }
                }
            }
        }
        DitherAlgorithm::FloydSteinberg
        | DitherAlgorithm::JarvisJudiceNinke
        | DitherAlgorithm::Stucki
        | DitherAlgorithm::Atkinson => {
            let mut buf: Vec<f32> = gray.iter().map(|&v| v as f32).collect();
            for y in 0..height {
                let line = pbm.line_mut(y);
                for x in 0..width {
                    let idx = y * width + x;
                    let old_val = buf[idx].clamp(0.0, 255.0);
                    let new_val = if old_val < 128.0 { 0.0 } else { 255.0 };
                    if new_val == 0.0 {
                        line[x / 8] |= 1 << (7 - (x % 8));
                        black_pixels += 1;
                    }
                    let err = old_val - new_val;
                    match cfg.algorithm {
                        DitherAlgorithm::FloydSteinberg => {
                            if x + 1 < width {
                                buf[idx + 1] += err * (7.0 / 16.0);
                            }
                            if y + 1 < height {
                                if x > 0 {
                                    buf[idx + width - 1] += err * (3.0 / 16.0);
                                }
                                buf[idx + width] += err * (5.0 / 16.0);
                                if x + 1 < width {
                                    buf[idx + width + 1] += err * (1.0 / 16.0);
                                }
                            }
                        }
                        DitherAlgorithm::Atkinson => {
                            let e8 = err / 8.0;
                            if x + 1 < width {
                                buf[idx + 1] += e8;
                            }
                            if x + 2 < width {
                                buf[idx + 2] += e8;
                            }
                            if y + 1 < height {
                                if x > 0 {
                                    buf[idx + width - 1] += e8;
                                }
                                buf[idx + width] += e8;
                                if x + 1 < width {
                                    buf[idx + width + 1] += e8;
                                }
                            }
                            if y + 2 < height {
                                buf[idx + 2 * width] += e8;
                            }
                        }
                        DitherAlgorithm::JarvisJudiceNinke => {
                            let e48 = err / 48.0;
                            let offsets: [(isize, isize, f32); 12] = [
                                (1, 0, 7.0),
                                (2, 0, 5.0),
                                (-2, 1, 3.0),
                                (-1, 1, 5.0),
                                (0, 1, 7.0),
                                (1, 1, 5.0),
                                (2, 1, 3.0),
                                (-2, 2, 1.0),
                                (-1, 2, 3.0),
                                (0, 2, 5.0),
                                (1, 2, 3.0),
                                (2, 2, 1.0),
                            ];
                            for (dx, dy, w) in offsets {
                                let nx = x as isize + dx;
                                let ny = y as isize + dy;
                                if nx >= 0 && nx < width as isize && ny < height as isize {
                                    buf[(ny as usize) * width + (nx as usize)] += e48 * w;
                                }
                            }
                        }
                        DitherAlgorithm::Stucki => {
                            let e42 = err / 42.0;
                            let offsets: [(isize, isize, f32); 12] = [
                                (1, 0, 8.0),
                                (2, 0, 4.0),
                                (-2, 1, 2.0),
                                (-1, 1, 4.0),
                                (0, 1, 8.0),
                                (1, 1, 4.0),
                                (2, 1, 2.0),
                                (-2, 2, 1.0),
                                (-1, 2, 2.0),
                                (0, 2, 4.0),
                                (1, 2, 2.0),
                                (2, 2, 1.0),
                            ];
                            for (dx, dy, w) in offsets {
                                let nx = x as isize + dx;
                                let ny = y as isize + dy;
                                if nx >= 0 && nx < width as isize && ny < height as isize {
                                    buf[(ny as usize) * width + (nx as usize)] += e42 * w;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    let elapsed = t0.elapsed().as_millis();
    let total_px = (width * height).max(1);
    (
        pbm,
        HalftoneBenchmarkResult {
            algorithm: cfg.algorithm,
            processing_ms: elapsed,
            black_pixels,
            coverage_percent: (black_pixels as f64) / (total_px as f64) * 100.0,
        },
    )
}
