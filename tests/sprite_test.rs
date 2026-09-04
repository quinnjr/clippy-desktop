use clippy_app::agent::Agent;
use clippy_app::sprite::{blend_over, premultiply, SpriteSheet};

#[test]
fn the_embedded_sheet_has_a_transparent_background() {
    let sheet = SpriteSheet::clippy();
    assert_eq!((sheet.width(), sheet.height()), (3348, 3162));
    assert_eq!(sheet.pixel(0, 0)[3], 0, "top-left corner is transparent");
    assert_eq!(
        sheet.pixel(123, 92)[3],
        0,
        "bottom-right of the rest frame is transparent"
    );
    assert_eq!(
        sheet.pixel(9999, 0),
        [0, 0, 0, 0],
        "outside the sheet reads as transparent"
    );
    let (mut transparent, mut opaque, mut partial) = (0u64, 0u64, 0u64);
    for y in 0..sheet.height() {
        for x in 0..sheet.width() {
            match sheet.pixel(x, y)[3] {
                0 => transparent += 1,
                255 => opaque += 1,
                _ => partial += 1,
            }
        }
    }
    assert_eq!(partial, 0, "the original sprites are hard-edged");
    assert!(transparent > opaque, "most of the sheet is background");
    assert!(opaque > 1_000_000, "the sheet is full of sprites");
}

#[test]
fn composing_the_rest_frame_gives_a_clippy_with_transparent_corners() {
    let agent = Agent::clippy();
    let sheet = SpriteSheet::clippy();
    let (w, h) = agent.frame_size();
    let cells = &agent.animation("RestPose").unwrap().frames[0].images;
    let mut out = Vec::new();
    sheet.compose(cells, w, h, &mut out);
    assert_eq!(out.len(), (w * h * 4) as usize);
    let alpha_at = |x: u32, y: u32| out[((y * w + x) * 4 + 3) as usize];
    assert_eq!(alpha_at(0, 0), 0);
    assert_eq!(alpha_at(w - 1, 0), 0);
    assert_eq!(alpha_at(0, h - 1), 0);
    assert_eq!(alpha_at(w - 1, h - 1), 0);
    let opaque = out.chunks_exact(4).filter(|px| px[3] == 255).count();
    assert!(opaque > 3000, "rest pose has {opaque} opaque pixels");
    for px in out.chunks_exact(4) {
        assert!(px[3] == 0 || px[3] == 255);
        if px[3] == 0 {
            assert_eq!(
                px,
                [0, 0, 0, 0],
                "transparent pixels are premultiplied to black"
            );
        }
    }
}

#[test]
fn composing_no_cells_gives_a_fully_transparent_frame() {
    let sheet = SpriteSheet::clippy();
    let mut out = vec![9u8; 4];
    sheet.compose(&[], 3, 2, &mut out);
    assert_eq!(out, vec![0u8; 24]);
}

#[test]
fn later_cells_are_drawn_over_earlier_ones() {
    let png = {
        let mut img = image::RgbaImage::new(4, 2);
        for x in 0..2 {
            for y in 0..2 {
                img.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
            }
        }
        img.put_pixel(2, 0, image::Rgba([0, 0, 255, 255]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        img.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        bytes.into_inner()
    };
    let sheet = SpriteSheet::decode_png(&png).unwrap();
    let mut out = Vec::new();
    sheet.compose(&[[0, 0], [2, 0]], 2, 2, &mut out);
    assert_eq!(
        &out[0..4],
        &[0, 0, 255, 255],
        "blue cell covers the red one"
    );
    assert_eq!(
        &out[4..8],
        &[255, 0, 0, 255],
        "red shows where the top cell is transparent"
    );
}

#[test]
fn premultiply_and_blend_follow_the_usual_formulas() {
    assert_eq!(premultiply([255, 255, 255, 0]), [0, 0, 0, 0]);
    assert_eq!(premultiply([200, 100, 50, 255]), [200, 100, 50, 255]);
    assert_eq!(premultiply([200, 100, 50, 128]), [100, 50, 25, 128]);
    assert_eq!(
        blend_over([0, 0, 0, 0], [10, 20, 30, 255]),
        [10, 20, 30, 255]
    );
    assert_eq!(
        blend_over([100, 100, 100, 255], [0, 0, 0, 0]),
        [100, 100, 100, 255]
    );
    // Half-transparent white over opaque black leaves mid grey.
    let mixed = blend_over([0, 0, 0, 255], [128, 128, 128, 128]);
    assert_eq!(mixed, [128, 128, 128, 255]);
}
