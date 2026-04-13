use std::time::{Duration, Instant};
use crate::animation::{AnimationState, Direction};
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

    /// Update behavior state (idle/walking transitions)
    pub fn update_behavior(&mut self) {
        let now = Instant::now();

        match self.animation_state {
            AnimationState::Idle => {
                // Check if it's time to make a behavior decision (2-5 seconds)
                if now.duration_since(self.behavior_timer) >= Duration::from_secs(2) {
                    let mut rng = rand::thread_rng();

                    // 60% chance to start walking
                    if rng.gen_bool(0.6) {
                        // Choose random direction
                        let direction = if rng.gen_bool(0.5) {
                            Direction::Left
                        } else {
                            Direction::Right
                        };

                        // Choose random walk duration (1-3 seconds)
                        let duration_ms = rng.gen_range(1000..=3000);

                        self.animation_state = AnimationState::Walking { direction };
                        self.walk_duration = Duration::from_millis(duration_ms);
                        self.walk_started = now;
                    }

                    // Reset behavior timer (2-5 second random interval)
                    let next_check_ms = rng.gen_range(2000..=5000);
                    self.behavior_timer = now - Duration::from_millis(rng.gen_range(0..next_check_ms));
                }
            }
            AnimationState::Walking { .. } => {
                // Check if walk duration has expired
                if now.duration_since(self.walk_started) >= self.walk_duration {
                    self.animation_state = AnimationState::Idle;
                    self.behavior_timer = now;
                }
            }
        }
    }

    /// Update position based on current animation state
    /// Movement speed: 3 pixels per frame at 60 FPS
    pub fn update_position(&mut self, screen_width: i32, screen_height: i32, sprite_size: i32) {
        if let AnimationState::Walking { direction } = self.animation_state {
            const SPEED: i32 = 3; // pixels per frame

            let (x, y) = self.position;
            let new_x = match direction {
                Direction::Left => x - SPEED,
                Direction::Right => x + SPEED,
            };

            // Clamp to screen bounds
            let clamped_x = new_x.clamp(0, (screen_width - sprite_size).max(0));
            let clamped_y = y.clamp(0, (screen_height - sprite_size).max(0));

            // If hit boundary, end walk early
            if clamped_x != new_x {
                self.animation_state = AnimationState::Idle;
                self.behavior_timer = Instant::now();
            }

            self.position = (clamped_x, clamped_y);
        }
    }
}
