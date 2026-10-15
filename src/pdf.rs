use crate::pbm::PbmImage;
use std::error::Error;
use windows::core::HSTRING;
use windows::Data::Pdf::{PdfDocument, PdfPageRenderOptions};
use windows::Graphics::Imaging::BitmapDecoder;
use windows::Storage::StorageFile;
use windows::Storage::Streams::InMemoryRandomAccessStream;

#[derive(Debug, Clone, Copy)]
pub struct PageRenderTiming {
    pub raster_ms: u128,
    pub pixel_ms: u128,
}

pub fn load_pdf_document(pdf_path: &str) -> Result<PdfDocument, Box<dyn Error>> {
    // Ensure COM / WinRT MTA is initialized on the calling thread (required for deterministic WinRT on Windows 10 & 11)
    unsafe {
        #[link(name = "ole32")]
        unsafe extern "system" {
            fn CoInitializeEx(pvReserved: *mut std::ffi::c_void, dwCoInit: u32) -> i32;
        }
        let _ = CoInitializeEx(std::ptr::null_mut(), 0); // 0 = COINIT_MULTITHREADED
    }

    let abs_path = std::fs::canonicalize(pdf_path)?
        .to_string_lossy()
        .replace(r"\\?\", "");
    let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(abs_path))?.join()?;
    let doc = PdfDocument::LoadFromFileAsync(&file)?.join()?;
    Ok(doc)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HalftoneMode {
    /// Normal mode: optimized for everyday text and documents
    /// - Solid text & dark strokes (Y <= 45) snap directly to solid black (100% crisp vector letters)
    /// - Clean margins (Y >= 245) snap directly to pure white (zero stray specks)
    /// - Midtones (46..244) use Serpentine Floyd-Steinberg so images & charts in documents are preserved
    Normal,
    /// Photo mode: full-range continuous Serpentine Floyd-Steinberg error diffusion for maximum photographic gradations
    Photo,
}

impl std::fmt::Display for HalftoneMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HalftoneMode::Normal => write!(f, "Normal"),
            HalftoneMode::Photo => write!(f, "Photo"),
        }
    }
}

#[allow(dead_code)]
pub fn render_pdf_page(doc: &PdfDocument, page_idx: u32) -> Result<(PbmImage, PageRenderTiming), Box<dyn Error>> {
    render_pdf_page_with_mode(doc, page_idx, HalftoneMode::Normal)
}

pub fn render_pdf_page_with_mode(
    doc: &PdfDocument,
    page_idx: u32,
    mode: HalftoneMode,
) -> Result<(PbmImage, PageRenderTiming), Box<dyn Error>> {
    let t_raster = std::time::Instant::now();
    let page_count = doc.PageCount()?;
    if page_idx >= page_count {
        return Err(format!("Page index {} exceeds document page count {}", page_idx, page_count).into());
    }

    let page = doc.GetPage(page_idx)?;
    let options = PdfPageRenderOptions::new()?;
    // Canonical width for Canon LBP-1120 A4 @ 600 DPI
    options.SetDestinationWidth(4736)?;

    let stream = InMemoryRandomAccessStream::new()?;
    page.RenderWithOptionsToStreamAsync(&stream, &options)?.join()?;

    // Rewind stream to beginning
    stream.Seek(0)?;

    let decoder = BitmapDecoder::CreateAsync(&stream)?.join()?;
    let pixel_data = decoder.GetPixelDataAsync()?.join()?;
    let raw_pixels = pixel_data.DetachPixelData()?;
    let raster_ms = t_raster.elapsed().as_millis();

    let width = decoder.PixelWidth()? as usize;
    let height = decoder.PixelHeight()? as usize;

    let t_pixel = std::time::Instant::now();
    // Convert RGBA pixels (4 bytes per pixel) to 1-bpp monochrome packed bitmap
    let target_width = 4736;
    let target_height = 6778;

    let mut pbm = PbmImage::new(target_width, target_height);

    let actual_h = height.min(target_height);
    let actual_w = width.min(target_width);

    let mut min_x = usize::MAX;
    let mut max_x = 0;
    let mut min_y = usize::MAX;
    let mut max_y = 0;
    let mut black_pixels = 0usize;

    let mut err_curr = vec![0i16; actual_w + 2];
    let mut err_next = vec![0i16; actual_w + 2];

    for y in 0..actual_h {
        let pbm_line = pbm.line_mut(y);
        let row_start = y * width * 4;

        if y % 2 == 0 {
            // Left to Right scan
            for x in 0..actual_w {
                let x_err = x + 1;
                let px_idx = row_start + x * 4;
                let r = raw_pixels[px_idx] as u32;
                let g = raw_pixels[px_idx + 1] as u32;
                let b = raw_pixels[px_idx + 2] as u32;
                let a = raw_pixels[px_idx + 3] as u32;

                let gray = if a == 255 {
                    (r * 299 + g * 587 + b * 114) / 1000
                } else if a == 0 {
                    255
                } else {
                    let eff_r = (r * a + 255 * (255 - a)) / 255;
                    let eff_g = (g * a + 255 * (255 - a)) / 255;
                    let eff_b = (b * a + 255 * (255 - a)) / 255;
                    (eff_r * 299 + eff_g * 587 + eff_b * 114) / 1000
                };

                let (is_black, error) = match mode {
                    HalftoneMode::Normal => {
                        if gray <= 45 {
                            // Hard snap: solid black text & vector lines (no dot diffusion on text)
                            (true, 0)
                        } else if gray >= 245 {
                            // Hard snap: clean white margins
                            (false, 0)
                        } else {
                            // Smooth error diffusion for midtones (photos and graphics in documents)
                            let val = gray as i32 + err_curr[x_err] as i32;
                            let val_clamped = val.clamp(0, 255);
                            if val_clamped < 135 {
                                (true, val_clamped)
                            } else {
                                (false, val_clamped - 255)
                            }
                        }
                    }
                    HalftoneMode::Photo => {
                        // Full-range photographic error diffusion with margin & deep shadow snapping
                        if gray <= 20 {
                            (true, 0)
                        } else if gray >= 242 {
                            (false, 0)
                        } else {
                            let val = gray as i32 + err_curr[x_err] as i32;
                            let val_clamped = val.clamp(0, 255);
                            if val_clamped < 128 {
                                (true, val_clamped)
                            } else {
                                (false, val_clamped - 255)
                            }
                        }
                    }
                };

                if is_black {
                    pbm_line[x / 8] |= 1 << (7 - (x % 8));
                    black_pixels += 1;
                    if x < min_x { min_x = x; }
                    if x > max_x { max_x = x; }
                    if y < min_y { min_y = y; }
                    if y > max_y { max_y = y; }
                }

                // Distribute error: 7/16 right, 3/16 down-left, 5/16 down, 1/16 down-right
                err_curr[x_err + 1] += (error * 7 / 16) as i16;
                err_next[x_err - 1] += (error * 3 / 16) as i16;
                err_next[x_err] += (error * 5 / 16) as i16;
                err_next[x_err + 1] += (error * 1 / 16) as i16;
            }
        } else {
            // Right to Left scan (Serpentine)
            for x in (0..actual_w).rev() {
                let x_err = x + 1;
                let px_idx = row_start + x * 4;
                let r = raw_pixels[px_idx] as u32;
                let g = raw_pixels[px_idx + 1] as u32;
                let b = raw_pixels[px_idx + 2] as u32;
                let a = raw_pixels[px_idx + 3] as u32;

                let gray = if a == 255 {
                    (r * 299 + g * 587 + b * 114) / 1000
                } else if a == 0 {
                    255
                } else {
                    let eff_r = (r * a + 255 * (255 - a)) / 255;
                    let eff_g = (g * a + 255 * (255 - a)) / 255;
                    let eff_b = (b * a + 255 * (255 - a)) / 255;
                    (eff_r * 299 + eff_g * 587 + eff_b * 114) / 1000
                };

                let (is_black, error) = match mode {
                    HalftoneMode::Normal => {
                        if gray <= 45 {
                            (true, 0)
                        } else if gray >= 245 {
                            (false, 0)
                        } else {
                            let val = gray as i32 + err_curr[x_err] as i32;
                            let val_clamped = val.clamp(0, 255);
                            if val_clamped < 135 {
                                (true, val_clamped)
                            } else {
                                (false, val_clamped - 255)
                            }
                        }
                    }
                    HalftoneMode::Photo => {
                        if gray <= 20 {
                            (true, 0)
                        } else if gray >= 242 {
                            (false, 0)
                        } else {
                            let val = gray as i32 + err_curr[x_err] as i32;
                            let val_clamped = val.clamp(0, 255);
                            if val_clamped < 128 {
                                (true, val_clamped)
                            } else {
                                (false, val_clamped - 255)
                            }
                        }
                    }
                };

                if is_black {
                    pbm_line[x / 8] |= 1 << (7 - (x % 8));
                    black_pixels += 1;
                    if x < min_x { min_x = x; }
                    if x > max_x { max_x = x; }
                    if y < min_y { min_y = y; }
                    if y > max_y { max_y = y; }
                }

                // Distribute error in reverse: 7/16 left, 3/16 down-right, 5/16 down, 1/16 down-left
                err_curr[x_err - 1] += (error * 7 / 16) as i16;
                err_next[x_err + 1] += (error * 3 / 16) as i16;
                err_next[x_err] += (error * 5 / 16) as i16;
                err_next[x_err - 1] += (error * 1 / 16) as i16;
            }
        }

        std::mem::swap(&mut err_curr, &mut err_next);
        err_next.fill(0);
    }

    let pixel_ms = t_pixel.elapsed().as_millis();

    if black_pixels > 0 {
        let bbox_w = max_x.saturating_sub(min_x) + 1;
        let bbox_h = max_y.saturating_sub(min_y) + 1;
        println!("  [Raster Analysis] Page {}/{} rendered: {}x{} | Black pixels: {} ({:.2}% coverage) | BBox: [{}, {}] -> [{}, {}] ({}x{} px)", 
            page_idx + 1, page_count, pbm.width, pbm.height, black_pixels, 
            (black_pixels as f64) / ((pbm.width * pbm.height) as f64) * 100.0,
            min_x, min_y, max_x, max_y, bbox_w, bbox_h);
    } else {
        println!("  [Raster Analysis] Page {}/{} rendered: {}x{} | BLANK PAGE (0 black pixels)!", 
            page_idx + 1, page_count, pbm.width, pbm.height);
    }

    Ok((pbm, PageRenderTiming { raster_ms, pixel_ms }))
}

#[allow(dead_code)]
pub fn render_pdf_file_to_pbm(pdf_path: &str, page_idx: u32) -> Result<(PbmImage, PageRenderTiming), Box<dyn Error>> {
    let doc = load_pdf_document(pdf_path)?;
    render_pdf_page(&doc, page_idx)
}

pub fn get_pdf_page_count(pdf_path: &str) -> Result<u32, Box<dyn Error>> {
    let doc = load_pdf_document(pdf_path)?;
    Ok(doc.PageCount()?)
}
