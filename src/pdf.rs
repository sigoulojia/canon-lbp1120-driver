use crate::pbm::PbmImage;
use std::error::Error;
use windows::Data::Pdf::{PdfDocument, PdfPageRenderOptions};
use windows::Graphics::Imaging::BitmapDecoder;
use windows::Storage::StorageFile;
use windows::Storage::Streams::InMemoryRandomAccessStream;
use windows::core::HSTRING;

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

pub const CLUSTERED_SCREEN_8X8: [[u8; 8]; 8] = [
    [230, 203, 175, 148, 152, 179, 207, 234],
    [226, 144, 115,  87,  91, 119, 148, 238],
    [199, 111,  58,  30,  34,  62, 123, 211],
    [171,  83,  26,   3,   7,  38,  95, 183],
    [167,  79,  22,   0,  11,  42,  99, 187],
    [195, 107,  54,  18,  14,  46, 127, 215],
    [222, 139,  75,  71,  50,  67, 135, 242],
    [246, 191, 163, 131, 103, 159, 175, 250],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HalftoneMode {
    /// Normal mode: optimized for everyday text and documents
    /// - Solid text & dark strokes (Y <= 55) snap directly to solid black (100% crisp vector letters)
    /// - Clean margins and light backgrounds (Y >= 225) snap directly to pure white (zero stray specks)
    /// - Midtones (56..224) use 8x8 clustered-dot halftone screening for optimal laser toner stability and high SCoA compression
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
pub fn render_pdf_page(
    doc: &PdfDocument,
    page_idx: u32,
) -> Result<(PbmImage, PageRenderTiming), Box<dyn Error>> {
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
        return Err(format!(
            "Page index {} exceeds document page count {}",
            page_idx, page_count
        )
        .into());
    }

    let page = doc.GetPage(page_idx)?;
    let page_size = page.Size()?;
    let page_w = page_size.Width as f64;
    let page_h = page_size.Height as f64;
    let is_landscape = page_w > page_h;

    let target_width = 4736u32;
    let target_height = 6778u32;

    let options = PdfPageRenderOptions::new()?;
    if is_landscape {
        let sw = (target_width as f64) / page_h;
        let sh = (target_height as f64) / page_w;
        let s = sw.min(sh);
        let dest_w = ((page_w * s).round() as u32).min(target_height);
        let dest_h = ((page_h * s).round() as u32).min(target_width);
        options.SetDestinationWidth(dest_w)?;
        options.SetDestinationHeight(dest_h)?;
    } else {
        let sw = (target_width as f64) / page_w;
        let sh = (target_height as f64) / page_h;
        let s = sw.min(sh);
        let dest_w = ((page_w * s).round() as u32).min(target_width);
        let dest_h = ((page_h * s).round() as u32).min(target_height);
        options.SetDestinationWidth(dest_w)?;
        options.SetDestinationHeight(dest_h)?;
    }

    let stream = InMemoryRandomAccessStream::new()?;
    page.RenderWithOptionsToStreamAsync(&stream, &options)?
        .join()?;

    // Rewind stream to beginning
    stream.Seek(0)?;

    let decoder = BitmapDecoder::CreateAsync(&stream)?.join()?;
    let pixel_data = decoder.GetPixelDataAsync()?.join()?;
    let raw_pixels = pixel_data.DetachPixelData()?;
    let raster_ms = t_raster.elapsed().as_millis();

    let width = decoder.PixelWidth()? as usize;
    let height = decoder.PixelHeight()? as usize;

    let (active_pixels, final_w, final_h) = if is_landscape {
        let final_w = height;
        let final_h = width;
        let mut rotated = vec![0u8; final_w * final_h * 4];
        for y_tile in (0..height).step_by(64) {
            let y_max = (y_tile + 64).min(height);
            for x_tile in (0..width).step_by(64) {
                let x_max = (x_tile + 64).min(width);
                for y in y_tile..y_max {
                    let src_row = y * width * 4;
                    let dst_x = y;
                    for x in x_tile..x_max {
                        let src_idx = src_row + x * 4;
                        let dst_y = width - 1 - x;
                        let dst_idx = (dst_y * final_w + dst_x) * 4;
                        rotated[dst_idx..dst_idx + 4]
                            .copy_from_slice(&raw_pixels[src_idx..src_idx + 4]);
                    }
                }
            }
        }
        (std::borrow::Cow::Owned(rotated), final_w, final_h)
    } else {
        (std::borrow::Cow::Borrowed(&raw_pixels[..]), width, height)
    };

    let t_pixel = std::time::Instant::now();
    // Convert RGBA pixels (4 bytes per pixel) to 1-bpp monochrome packed bitmap
    let target_width = 4736usize;
    let target_height = 6778usize;

    let mut pbm = PbmImage::new(target_width, target_height);

    let actual_h = final_h.min(target_height);
    let actual_w = final_w.min(target_width);

    let x_offset = (target_width.saturating_sub(actual_w)) / 2;
    let y_offset = (target_height.saturating_sub(actual_h)) / 2;

    let mut min_x = usize::MAX;
    let mut max_x = 0;
    let mut min_y = usize::MAX;
    let mut max_y = 0;
    let mut black_pixels = 0usize;

    let mut err_curr = vec![0i16; actual_w + 2];
    let mut err_next = vec![0i16; actual_w + 2];

    for y in 0..actual_h {
        let dest_y = y + y_offset;
        let pbm_line = pbm.line_mut(dest_y);
        let row_start = y * final_w * 4;

        if y % 2 == 0 {
            // Left to Right scan
            for x in 0..actual_w {
                let x_err = x + 1;
                let px_idx = row_start + x * 4;
                let r = active_pixels[px_idx] as u32;
                let g = active_pixels[px_idx + 1] as u32;
                let b = active_pixels[px_idx + 2] as u32;
                let a = active_pixels[px_idx + 3] as u32;

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
                        if gray <= 55 {
                            // Hard snap: solid black text & vector lines (no dot diffusion on text)
                            (true, 0)
                        } else if gray >= 225 {
                            // Hard snap: clean white margins & light background tints
                            (false, 0)
                        } else {
                            // 8x8 clustered-dot halftone screen for graphics/charts/images
                            let threshold = CLUSTERED_SCREEN_8X8[dest_y % 8][x % 8];
                            let is_dot = (gray as u8) < threshold;
                            (is_dot, 0)
                        }
                    }
                    HalftoneMode::Photo => {
                        // Full-range photographic error diffusion with margin & deep shadow snapping
                        if gray <= 20 {
                            (true, 0)
                        } else if gray >= 235 {
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
                    let dest_x = x + x_offset;
                    pbm_line[dest_x / 8] |= 1 << (7 - (dest_x % 8));
                    black_pixels += 1;
                    if dest_x < min_x {
                        min_x = dest_x;
                    }
                    if dest_x > max_x {
                        max_x = dest_x;
                    }
                    if dest_y < min_y {
                        min_y = dest_y;
                    }
                    if dest_y > max_y {
                        max_y = dest_y;
                    }
                }

                // Distribute error: 7/16 right, 3/16 down-left, 5/16 down, 1/16 down-right
                err_curr[x_err + 1] += (error * 7 / 16) as i16;
                err_next[x_err - 1] += (error * 3 / 16) as i16;
                err_next[x_err] += (error * 5 / 16) as i16;
                err_next[x_err + 1] += (error / 16) as i16;
            }
        } else {
            // Right to Left scan (Serpentine)
            for x in (0..actual_w).rev() {
                let x_err = x + 1;
                let px_idx = row_start + x * 4;
                let r = active_pixels[px_idx] as u32;
                let g = active_pixels[px_idx + 1] as u32;
                let b = active_pixels[px_idx + 2] as u32;
                let a = active_pixels[px_idx + 3] as u32;

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
                        if gray <= 55 {
                            // Hard snap: solid black text & vector lines (no dot diffusion on text)
                            (true, 0)
                        } else if gray >= 225 {
                            // Hard snap: clean white margins & light background tints
                            (false, 0)
                        } else {
                            // 8x8 clustered-dot halftone screen for graphics/charts/images
                            let threshold = CLUSTERED_SCREEN_8X8[dest_y % 8][x % 8];
                            let is_dot = (gray as u8) < threshold;
                            (is_dot, 0)
                        }
                    }
                    HalftoneMode::Photo => {
                        if gray <= 20 {
                            (true, 0)
                        } else if gray >= 235 {
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
                    let dest_x = x + x_offset;
                    pbm_line[dest_x / 8] |= 1 << (7 - (dest_x % 8));
                    black_pixels += 1;
                    if dest_x < min_x {
                        min_x = dest_x;
                    }
                    if dest_x > max_x {
                        max_x = dest_x;
                    }
                    if dest_y < min_y {
                        min_y = dest_y;
                    }
                    if dest_y > max_y {
                        max_y = dest_y;
                    }
                }

                // Distribute error in reverse: 7/16 left, 3/16 down-right, 5/16 down, 1/16 down-left
                err_curr[x_err - 1] += (error * 7 / 16) as i16;
                err_next[x_err + 1] += (error * 3 / 16) as i16;
                err_next[x_err] += (error * 5 / 16) as i16;
                err_next[x_err - 1] += (error / 16) as i16;
            }
        }

        std::mem::swap(&mut err_curr, &mut err_next);
        err_next.fill(0);
    }

    let pixel_ms = t_pixel.elapsed().as_millis();

    if black_pixels > 0 {
        let bbox_w = max_x.saturating_sub(min_x) + 1;
        let bbox_h = max_y.saturating_sub(min_y) + 1;
        println!(
            "  [Raster Analysis] Page {}/{} rendered: {}x{} | Black pixels: {} ({:.2}% coverage) | BBox: [{}, {}] -> [{}, {}] ({}x{} px)",
            page_idx + 1,
            page_count,
            pbm.width,
            pbm.height,
            black_pixels,
            (black_pixels as f64) / ((pbm.width * pbm.height) as f64) * 100.0,
            min_x,
            min_y,
            max_x,
            max_y,
            bbox_w,
            bbox_h
        );
    } else {
        println!(
            "  [Raster Analysis] Page {}/{} rendered: {}x{} | BLANK PAGE (0 black pixels)!",
            page_idx + 1,
            page_count,
            pbm.width,
            pbm.height
        );
    }

    Ok((
        pbm,
        PageRenderTiming {
            raster_ms,
            pixel_ms,
        },
    ))
}

#[allow(dead_code)]
pub fn render_pdf_file_to_pbm(
    pdf_path: &str,
    page_idx: u32,
) -> Result<(PbmImage, PageRenderTiming), Box<dyn Error>> {
    let doc = load_pdf_document(pdf_path)?;
    render_pdf_page(&doc, page_idx)
}

pub fn get_pdf_page_count(pdf_path: &str) -> Result<u32, Box<dyn Error>> {
    let doc = load_pdf_document(pdf_path)?;
    Ok(doc.PageCount()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    pub fn create_synthetic_pdf(
        path: &std::path::Path,
        pages: &[(f64, f64, &str)], // (width, height, content_ops)
        leading_bytes: &[u8],
    ) -> std::io::Result<()> {
        let mut f = File::create(path)?;
        f.write_all(leading_bytes)?;

        let mut offsets = Vec::new();
        let mut cur_offset = leading_bytes.len();

        let header = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n";
        f.write_all(header)?;
        cur_offset += header.len();

        // Object 1: Catalog
        offsets.push(cur_offset);
        let obj1 = b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n";
        f.write_all(obj1)?;
        cur_offset += obj1.len();

        // Object 2: Pages
        let mut kids = String::new();
        for i in 0..pages.len() {
            kids.push_str(&format!("{} 0 R ", 3 + i * 3));
        }
        let obj2 = format!(
            "2 0 obj\n<< /Type /Pages /Kids [{}] /Count {} >>\nendobj\n",
            kids.trim(),
            pages.len()
        );
        offsets.push(cur_offset);
        f.write_all(obj2.as_bytes())?;
        cur_offset += obj2.len();

        for (i, (w, h, stream_ops)) in pages.iter().enumerate() {
            let page_obj_id = 3 + i * 3;
            let contents_obj_id = 4 + i * 3;
            let font_obj_id = 5 + i * 3;

            // Page obj
            offsets.push(cur_offset);
            let page_obj = format!(
                "{} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] /Contents {} 0 R /Resources << /Font << /F1 {} 0 R >> >> >>\nendobj\n",
                page_obj_id, w, h, contents_obj_id, font_obj_id
            );
            f.write_all(page_obj.as_bytes())?;
            cur_offset += page_obj.len();

            // Contents obj
            offsets.push(cur_offset);
            let contents_obj = format!(
                "{} 0 obj\n<< /Length {} >>\nstream\n{}\nendstream\nendobj\n",
                contents_obj_id,
                stream_ops.len(),
                stream_ops
            );
            f.write_all(contents_obj.as_bytes())?;
            cur_offset += contents_obj.len();

            // Font obj
            offsets.push(cur_offset);
            let font_obj = format!(
                "{} 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n",
                font_obj_id
            );
            f.write_all(font_obj.as_bytes())?;
            cur_offset += font_obj.len();
        }

        // XRef table
        let xref_offset = cur_offset;
        let total_objs = 1 + offsets.len();
        let mut xref = format!("xref\n0 {}\n0000000000 65535 f \r\n", total_objs);
        for off in &offsets {
            xref.push_str(&format!("{:010} 00000 n \r\n", off));
        }
        xref.push_str(&format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            total_objs, xref_offset
        ));
        f.write_all(xref.as_bytes())?;
        Ok(())
    }

    #[test]
    fn test_render_a4_portrait_text() {
        let temp = std::env::temp_dir().join("test_a4_portrait.pdf");
        let stream =
            "BT /F1 28 Tf 100 700 Td (Canon LBP-1120 Portrait Text) Tj ET 100 500 200 80 re f";
        create_synthetic_pdf(&temp, &[(595.28, 841.89, stream)], b"").expect("write pdf");

        let doc = load_pdf_document(&temp.to_string_lossy()).expect("load pdf");
        assert_eq!(doc.PageCount().unwrap(), 1);

        let (pbm, _) = render_pdf_page_with_mode(&doc, 0, HalftoneMode::Normal).expect("render");
        assert_eq!(pbm.width, 4736);
        assert_eq!(pbm.height, 6778);

        // Verify black pixels exist (text and rectangle rendered)
        let total_black: usize = pbm.data.iter().map(|b| b.count_ones() as usize).sum();
        assert!(
            total_black > 1000,
            "Expected text and rectangle pixels, found {}",
            total_black
        );

        let _ = std::fs::remove_file(temp);
    }

    #[test]
    fn test_render_a4_landscape_auto_rotation() {
        let temp = std::env::temp_dir().join("test_a4_landscape.pdf");
        let stream =
            "BT /F1 32 Tf 100 450 Td (Landscape Mode Auto-Rotated) Tj ET 100 200 300 60 re f";
        create_synthetic_pdf(&temp, &[(841.89, 595.28, stream)], b"").expect("write pdf");

        let doc = load_pdf_document(&temp.to_string_lossy()).expect("load pdf");
        assert_eq!(doc.PageCount().unwrap(), 1);

        let (pbm, _) = render_pdf_page_with_mode(&doc, 0, HalftoneMode::Normal).expect("render");
        assert_eq!(pbm.width, 4736);
        assert_eq!(pbm.height, 6778);

        let total_black: usize = pbm.data.iter().map(|b| b.count_ones() as usize).sum();
        assert!(
            total_black > 1000,
            "Expected landscape pixels rendered, found {}",
            total_black
        );

        let _ = std::fs::remove_file(temp);
    }

    #[test]
    fn test_render_us_legal_portrait_proportional_fit() {
        let temp = std::env::temp_dir().join("test_us_legal.pdf");
        // Place shapes near bottom (y=50) and top (y=950) of Legal page (height=1008)
        let stream = "50 50 150 40 re f 50 950 150 40 re f";
        create_synthetic_pdf(&temp, &[(612.0, 1008.0, stream)], b"").expect("write pdf");

        let doc = load_pdf_document(&temp.to_string_lossy()).expect("load pdf");
        let (pbm, _) = render_pdf_page_with_mode(&doc, 0, HalftoneMode::Normal).expect("render");
        assert_eq!(pbm.width, 4736);
        assert_eq!(pbm.height, 6778);

        // Find bounding box to prove both top and bottom shapes are preserved without clipping
        let mut min_y = usize::MAX;
        let mut max_y = 0;
        for y in 0..pbm.height {
            let line = pbm.line(y);
            if line.iter().any(|&b| b != 0) {
                if y < min_y {
                    min_y = y;
                }
                if y > max_y {
                    max_y = y;
                }
            }
        }
        assert!(min_y < 1000, "Top marker missing, min_y: {}", min_y);
        assert!(max_y > 5500, "Bottom marker clipped, max_y: {}", max_y);

        let _ = std::fs::remove_file(temp);
    }

    #[test]
    fn test_render_multi_page_pdf() {
        let temp = std::env::temp_dir().join("test_multi_page.pdf");
        let p1 = "BT /F1 24 Tf 100 700 Td (Page 1 Content) Tj ET";
        let p2 = "BT /F1 24 Tf 100 700 Td (Page 2 Unique Content) Tj ET 100 400 100 100 re f";
        let p3 = "BT /F1 24 Tf 100 700 Td (Page 3 Distinct Content) Tj ET 100 300 200 50 re f";
        create_synthetic_pdf(
            &temp,
            &[
                (595.28, 841.89, p1),
                (595.28, 841.89, p2),
                (595.28, 841.89, p3),
            ],
            b"",
        )
        .expect("write multi-page pdf");

        let count = get_pdf_page_count(&temp.to_string_lossy()).expect("get page count");
        assert_eq!(count, 3);

        let doc = load_pdf_document(&temp.to_string_lossy()).expect("load");
        let (pbm1, _) =
            render_pdf_page_with_mode(&doc, 0, HalftoneMode::Normal).expect("render p1");
        let (pbm2, _) =
            render_pdf_page_with_mode(&doc, 1, HalftoneMode::Normal).expect("render p2");
        let (pbm3, _) =
            render_pdf_page_with_mode(&doc, 2, HalftoneMode::Normal).expect("render p3");

        assert_eq!(pbm1.width, 4736);
        assert_eq!(pbm2.width, 4736);
        assert_eq!(pbm3.width, 4736);

        // Verify each page rendered distinct data
        let b1: usize = pbm1.data.iter().map(|b| b.count_ones() as usize).sum();
        let b2: usize = pbm2.data.iter().map(|b| b.count_ones() as usize).sum();
        let b3: usize = pbm3.data.iter().map(|b| b.count_ones() as usize).sum();
        assert!(b1 > 0 && b2 > 0 && b3 > 0);
        assert_ne!(
            b1, b2,
            "Page 1 and Page 2 should have different pixel counts"
        );
        assert_ne!(
            b2, b3,
            "Page 2 and Page 3 should have different pixel counts"
        );

        let _ = std::fs::remove_file(temp);
    }

    #[test]
    fn test_render_corrupt_truncated_pdf() {
        let temp = std::env::temp_dir().join("test_corrupt.pdf");
        {
            let mut f = File::create(&temp).expect("create");
            // Write partial header and truncated body
            f.write_all(b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog >>\n")
                .expect("write");
        }

        // Must produce clear error, not panic
        let res = load_pdf_document(&temp.to_string_lossy());
        assert!(res.is_err(), "Corrupt PDF should produce an error");

        let _ = std::fs::remove_file(temp);
    }

    #[test]
    fn test_render_halftone_mode_comparison() {
        let temp = std::env::temp_dir().join("test_halftone_modes.pdf");
        // Draw gray boxes (gray 0.5 and gray 0.2)
        let stream = "0.5 g 100 500 200 100 re f 0.2 g 100 300 200 100 re f";
        create_synthetic_pdf(&temp, &[(595.28, 841.89, stream)], b"").expect("write pdf");

        let doc = load_pdf_document(&temp.to_string_lossy()).expect("load");
        let (pbm_norm, _) =
            render_pdf_page_with_mode(&doc, 0, HalftoneMode::Normal).expect("render normal");
        let (pbm_photo, _) =
            render_pdf_page_with_mode(&doc, 0, HalftoneMode::Photo).expect("render photo");

        assert_eq!(pbm_norm.width, 4736);
        assert_eq!(pbm_photo.width, 4736);

        let black_norm: usize = pbm_norm.data.iter().map(|b| b.count_ones() as usize).sum();
        let black_photo: usize = pbm_photo.data.iter().map(|b| b.count_ones() as usize).sum();

        assert!(black_norm > 0);
        assert!(black_photo > 0);
        // Normal mode snaps solid black/white thresholds whereas Photo diffuses continuous tones
        assert_ne!(black_norm, black_photo);

        let _ = std::fs::remove_file(temp);
    }
}
