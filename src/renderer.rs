use softbuffer::{Context, Surface};
use std::num::NonZeroU32;
use winit::window::Window;
use image::RgbaImage;

pub struct Renderer {
    surface: Surface<std::sync::Arc<Window>, std::sync::Arc<Window>>,
}

impl Renderer {
    pub fn new(window: std::sync::Arc<Window>) -> Result<Self, Box<dyn std::error::Error>> {
        let context = Context::new(window.clone())?;
        let surface = Surface::new(&context, window)?;

        Ok(Self { surface })
    }

    pub fn render(&mut self, sprite: &RgbaImage, window_size: (u32, u32)) -> Result<(), Box<dyn std::error::Error>> {
        let (width, height) = window_size;

        self.surface.resize(
            NonZeroU32::new(width).unwrap(),
            NonZeroU32::new(height).unwrap(),
        )?;

        let mut buffer = self.surface.buffer_mut()?;

        // Copy sprite data to buffer
        for y in 0..height.min(sprite.height()) {
            for x in 0..width.min(sprite.width()) {
                let pixel = sprite.get_pixel(x, y);
                let rgba = pixel.0;

                // Convert RGBA to u32 (ARGB format for most platforms)
                let color = u32::from_be_bytes([rgba[3], rgba[0], rgba[1], rgba[2]]);

                let index = (y * width + x) as usize;
                if index < buffer.len() {
                    buffer[index] = color;
                }
            }
        }

        buffer.present()?;
        Ok(())
    }
}
