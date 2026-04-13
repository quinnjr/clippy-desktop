use std::time::{Duration, Instant};
use crate::animation::AnimationState;
use rand::Rng;

pub struct CompanionState {
    pub position: (i32, i32),
    pub animation_state: AnimationState,
    pub current_frame: usize,
    pub frame_timer: Instant,
    pub behavior_timer: Instant,
    pub walk_duration: Duration,
    pub walk_started: Instant,
}

impl CompanionState {
    pub fn new(screen_width: i32, screen_height: i32, sprite_size: i32) -> Self {
        let mut rng = rand::thread_rng();

        // Random starting position within bounds
        let x = rng.gen_range(0..=(screen_width - sprite_size).max(0));
        let y = rng.gen_range(0..=(screen_height - sprite_size).max(0));

        let now = Instant::now();

        Self {
            position: (x, y),
            animation_state: AnimationState::Idle,
            current_frame: 0,
            frame_timer: now,
            behavior_timer: now,
            walk_duration: Duration::from_secs(0),
            walk_started: now,
        }
    }
}
