use image::RgbaImage;
use crate::animation::{AnimationState, Direction};

pub struct SpriteSheet {
    pub idle_frames: Vec<RgbaImage>,
    pub walk_left_frames: Vec<RgbaImage>,
    pub walk_right_frames: Vec<RgbaImage>,
}

impl SpriteSheet {
    /// Load sprites embedded in binary at compile time
    pub fn load_embedded() -> Self {
        let idle = Self::load_image_from_bytes(include_bytes!("../assets/sprites/idle.png"))
            .expect("Failed to load idle.png");

        let walk_left_frames = (0..4)
            .map(|i| {
                let bytes: &[u8] = match i {
                    0 => include_bytes!("../assets/sprites/walk_left_0.png"),
                    1 => include_bytes!("../assets/sprites/walk_left_1.png"),
                    2 => include_bytes!("../assets/sprites/walk_left_2.png"),
                    3 => include_bytes!("../assets/sprites/walk_left_3.png"),
                    _ => unreachable!(),
                };
                Self::load_image_from_bytes(bytes)
                    .expect(&format!("Failed to load walk_left_{}.png", i))
            })
            .collect();

        // For walk_right, horizontally flip walk_left frames
        let walk_right_frames = (0..4)
            .map(|i| {
                let bytes: &[u8] = match i {
                    0 => include_bytes!("../assets/sprites/walk_left_0.png"),
                    1 => include_bytes!("../assets/sprites/walk_left_1.png"),
                    2 => include_bytes!("../assets/sprites/walk_left_2.png"),
                    3 => include_bytes!("../assets/sprites/walk_left_3.png"),
                    _ => unreachable!(),
                };
                let img = Self::load_image_from_bytes(bytes)
                    .expect(&format!("Failed to load walk_left_{}.png for flipping", i));
                image::imageops::flip_horizontal(&img)
            })
            .collect();

        Self {
            idle_frames: vec![idle],
            walk_left_frames,
            walk_right_frames,
        }
    }

    fn load_image_from_bytes(bytes: &[u8]) -> Result<RgbaImage, image::ImageError> {
        let img = image::load_from_memory(bytes)?;
        Ok(img.to_rgba8())
    }

    /// Get frame for given animation state and frame index
    pub fn get_frame(&self, state: &AnimationState, frame_index: usize) -> Option<&RgbaImage> {
        let frames = match state {
            AnimationState::Idle => &self.idle_frames,
            AnimationState::Walking {
                direction: Direction::Left,
            } => &self.walk_left_frames,
            AnimationState::Walking {
                direction: Direction::Right,
            } => &self.walk_right_frames,
        };

        if frames.is_empty() {
            return None;
        }

        // Wrap frame index to array bounds
        let index = frame_index % frames.len();
        Some(&frames[index])
    }

    /// Get dimensions of sprites (assumes all frames same size)
    pub fn sprite_dimensions(&self) -> (u32, u32) {
        if let Some(frame) = self.idle_frames.first() {
            (frame.width(), frame.height())
        } else {
            (128, 128) // Default
        }
    }
}
