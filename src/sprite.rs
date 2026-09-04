//! Sprite sheet decoding and per-frame compositing.
//!
//! The sheet is kept as straight-alpha RGBA exactly as decoded. Frames are
//! composited on demand into a small premultiplied-alpha buffer, which is
//! what every desktop compositor expects from a transparent window.

/// The Clippy sprite sheet: an RGBA PNG with a fully transparent background.
pub const CLIPPY_SHEET_PNG: &[u8] = include_bytes!("../assets/clippy/map.png");

pub struct SpriteSheet {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl SpriteSheet {
    /// Decodes a PNG sprite sheet into straight-alpha RGBA.
    pub fn decode_png(bytes: &[u8]) -> Result<Self, image::ImageError> {
        let image =
            image::load_from_memory_with_format(bytes, image::ImageFormat::Png)?.into_rgba8();
        Ok(Self {
            width: image.width(),
            height: image.height(),
            rgba: image.into_raw(),
        })
    }

    /// The embedded Clippy sheet.
    pub fn clippy() -> Self {
        Self::decode_png(CLIPPY_SHEET_PNG).expect("embedded map.png is a valid PNG")
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Straight-alpha RGBA of one sheet pixel; transparent black outside the sheet.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        if x >= self.width || y >= self.height {
            return [0, 0, 0, 0];
        }
        let offset = ((y * self.width + x) * 4) as usize;
        let px = &self.rgba[offset..offset + 4];
        [px[0], px[1], px[2], px[3]]
    }

    /// Composites the given sheet cells (bottom first) into `out` as
    /// premultiplied RGBA, `frame_width * frame_height * 4` bytes, top row first.
    pub fn compose(
        &self,
        cells: &[[u32; 2]],
        frame_width: u32,
        frame_height: u32,
        out: &mut Vec<u8>,
    ) {
        let len = (frame_width * frame_height * 4) as usize;
        out.clear();
        out.resize(len, 0);
        for &[cell_x, cell_y] in cells {
            for y in 0..frame_height {
                for x in 0..frame_width {
                    let src = premultiply(self.pixel(cell_x + x, cell_y + y));
                    if src[3] == 0 {
                        continue;
                    }
                    let offset = ((y * frame_width + x) * 4) as usize;
                    let dst: &mut [u8] = &mut out[offset..offset + 4];
                    let blended = blend_over([dst[0], dst[1], dst[2], dst[3]], src);
                    dst.copy_from_slice(&blended);
                }
            }
        }
    }
}

/// Converts straight-alpha RGBA to premultiplied RGBA.
pub fn premultiply([r, g, b, a]: [u8; 4]) -> [u8; 4] {
    if a == 255 {
        return [r, g, b, a];
    }
    let mul = |c: u8| ((c as u32 * a as u32 + 127) / 255) as u8;
    [mul(r), mul(g), mul(b), a]
}

/// Source-over blend of two premultiplied RGBA pixels.
pub fn blend_over(dst: [u8; 4], src: [u8; 4]) -> [u8; 4] {
    let inv = 255 - src[3] as u32;
    let mix = |s: u8, d: u8| (s as u32 + (d as u32 * inv + 127) / 255).min(255) as u8;
    [
        mix(src[0], dst[0]),
        mix(src[1], dst[1]),
        mix(src[2], dst[2]),
        mix(src[3], dst[3]),
    ]
}
