//! PNG alpha decoding and opaque-pixel bounds, relative to each frame's center.

use crate::animdata::AnimTable;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoxI {
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32, // exclusive
    pub max_y: i32, // exclusive
}

impl BoxI {
    pub fn width(&self) -> i32 { self.max_x - self.min_x }
    pub fn height(&self) -> i32 { self.max_y - self.min_y }
    pub fn union(&self, o: &BoxI) -> BoxI {
        BoxI {
            min_x: self.min_x.min(o.min_x),
            min_y: self.min_y.min(o.min_y),
            max_x: self.max_x.max(o.max_x),
            max_y: self.max_y.max(o.max_y),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AlphaSheet {
    pub width: u32,
    pub height: u32,
    /// One byte per pixel, row-major, top row first.
    pub alpha: Vec<u8>,
}

pub fn decode_alpha(bytes: &[u8]) -> Result<AlphaSheet, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16 | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("PNG too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let channels = match info.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::GrayscaleAlpha => 2,
        other => return Err(format!("unexpected PNG color type {other:?}")),
    };
    let mut alpha = Vec::with_capacity((info.width * info.height) as usize);
    for y in 0..info.height as usize {
        let row = &buf[y * info.line_size..];
        for x in 0..info.width as usize {
            alpha.push(row[x * channels + channels - 1]);
        }
    }
    Ok(AlphaSheet { width: info.width, height: info.height, alpha })
}

/// Opaque bounds of the cell at (x0, y0), relative to the cell center (fw/2, fh/2).
pub fn frame_bounds(sheet: &AlphaSheet, x0: u32, y0: u32, fw: u32, fh: u32) -> Option<BoxI> {
    let mut b: Option<BoxI> = None;
    let (cx, cy) = ((fw / 2) as i32, (fh / 2) as i32);
    for y in 0..fh {
        for x in 0..fw {
            if sheet.alpha[((y0 + y) * sheet.width + x0 + x) as usize] == 0 {
                continue;
            }
            let p = BoxI { min_x: x as i32 - cx, min_y: y as i32 - cy, max_x: x as i32 - cx + 1, max_y: y as i32 - cy + 1 };
            b = Some(b.map_or(p, |b| b.union(&p)));
        }
    }
    b
}

/// Union of opaque pixels over every frame and row of the named animations that exist.
pub fn content_box(table: &AnimTable, sheets: &HashMap<String, AlphaSheet>, names: &[String]) -> BoxI {
    let mut b: Option<BoxI> = None;
    for name in names {
        let Some(anim) = table.get(name) else { continue };
        let Some(sheet) = sheets.get(&anim.source) else { continue };
        for row in 0..anim.rows {
            for col in 0..anim.durations.len() as u32 {
                if let Some(f) = frame_bounds(sheet, col * anim.frame_width, row * anim.frame_height, anim.frame_width, anim.frame_height) {
                    b = Some(b.map_or(f, |b| b.union(&f)));
                }
            }
        }
    }
    b.unwrap_or(BoxI { min_x: -1, min_y: -1, max_x: 1, max_y: 1 })
}

/// The standing foot line: the lowest opaque row of Walk and Idle, relative to the frame center.
/// Pets are placed so this row rests on the ground.
pub fn feet_line(table: &AnimTable, sheets: &HashMap<String, AlphaSheet>) -> i32 {
    content_box(table, sheets, &["Walk".to_string(), "Idle".to_string()]).max_y
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// RGBA PNG of `w`x`h`; `opaque(x, y)` decides which pixels have alpha 255.
    pub fn png_bytes(w: u32, h: u32, opaque: impl Fn(u32, u32) -> bool) -> Vec<u8> {
        let mut data = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                data.extend_from_slice(&[10, 20, 30, if opaque(x, y) { 255 } else { 0 }]);
            }
        }
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, w, h);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut writer = enc.write_header().unwrap();
            writer.write_image_data(&data).unwrap();
            writer.finish().unwrap();
        }
        out
    }

    #[test]
    fn decodes_alpha_plane() {
        let sheet = decode_alpha(&png_bytes(4, 2, |x, y| x == 1 && y == 1)).unwrap();
        assert_eq!((sheet.width, sheet.height), (4, 2));
        assert_eq!(sheet.alpha, [0, 0, 0, 0, 0, 255, 0, 0]);
    }

    #[test]
    fn rejects_garbage() {
        assert!(decode_alpha(b"not a png").is_err());
        let mut truncated = png_bytes(8, 8, |_, _| true);
        truncated.truncate(truncated.len() / 2);
        assert!(decode_alpha(&truncated).is_err());
    }

    #[test]
    fn frame_bounds_are_relative_to_cell_center() {
        // One 8x8 cell; opaque block at x 2..5, y 3..8 → relative to center (4,4): x -2..1, y -1..4
        let sheet = decode_alpha(&png_bytes(8, 8, |x, y| (2..5).contains(&x) && (3..8).contains(&y))).unwrap();
        assert_eq!(frame_bounds(&sheet, 0, 0, 8, 8), Some(BoxI { min_x: -2, min_y: -1, max_x: 1, max_y: 4 }));
        let empty = decode_alpha(&png_bytes(8, 8, |_, _| false)).unwrap();
        assert_eq!(frame_bounds(&empty, 0, 0, 8, 8), None);
    }

    #[test]
    fn feet_line_is_the_lowest_walk_or_idle_pixel() {
        let walk = png_bytes(8, 8, |_, y| y < 6); // opaque rows 0..6 → max_y 6-4 = 2
        let idle = png_bytes(8, 8, |_, y| y < 7); // max_y 3
        let swing = png_bytes(8, 8, |_, _| true); // max_y 4, ignored
        let mut table = crate::animdata::parse(r#"<AnimData><Anims>
            <Anim><Name>Walk</Name><FrameWidth>8</FrameWidth><FrameHeight>8</FrameHeight><Durations><Duration>1</Duration></Durations></Anim>
            <Anim><Name>Idle</Name><FrameWidth>8</FrameWidth><FrameHeight>8</FrameHeight><Durations><Duration>1</Duration></Durations></Anim>
            <Anim><Name>Swing</Name><FrameWidth>8</FrameWidth><FrameHeight>8</FrameHeight><Durations><Duration>1</Duration></Durations></Anim>
            </Anims></AnimData>"#).unwrap();
        let mut sheets = HashMap::new();
        sheets.insert("Walk".to_string(), decode_alpha(&walk).unwrap());
        sheets.insert("Idle".to_string(), decode_alpha(&idle).unwrap());
        sheets.insert("Swing".to_string(), decode_alpha(&swing).unwrap());
        table.validate(|s| sheets.get(s).map(|a| (a.width, a.height))).unwrap();
        assert_eq!(feet_line(&table, &sheets), 3);
    }

    #[test]
    fn content_box_unions_frames_rows_and_named_anims_only() {
        // Walk: 2 columns x 2 rows of 8x8. Frame (col 1,row 1) has a pixel at its top-left corner.
        let walk = png_bytes(16, 16, |x, y| (x, y) == (3, 4) || (x, y) == (8, 8));
        let other = png_bytes(8, 8, |_, _| true);
        let mut table = crate::animdata::parse(r#"<AnimData><Anims>
            <Anim><Name>Walk</Name><FrameWidth>8</FrameWidth><FrameHeight>8</FrameHeight><Durations><Duration>1</Duration><Duration>1</Duration></Durations></Anim>
            <Anim><Name>Big</Name><FrameWidth>8</FrameWidth><FrameHeight>8</FrameHeight><Durations><Duration>1</Duration></Durations></Anim>
            </Anims></AnimData>"#).unwrap();
        let mut sheets = HashMap::new();
        sheets.insert("Walk".to_string(), decode_alpha(&walk).unwrap());
        sheets.insert("Big".to_string(), decode_alpha(&other).unwrap());
        table.validate(|s| sheets.get(s).map(|a| (a.width, a.height))).unwrap();
        let b = content_box(&table, &sheets, &["Walk".to_string(), "Missing".to_string()]);
        assert_eq!(b, BoxI { min_x: -4, min_y: -4, max_x: 0, max_y: 1 });
        assert_eq!((b.width(), b.height()), (4, 5));
    }
}
