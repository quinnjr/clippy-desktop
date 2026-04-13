use clippy_app::companion::CompanionState;
use clippy_app::animation::AnimationState;
use std::time::Duration;

#[test]
fn test_companion_state_new() {
    let screen_width = 1920;
    let screen_height = 1080;
    let sprite_size = 128;

    let state = CompanionState::new(screen_width, screen_height, sprite_size);

    // Should start in Idle state
    assert!(matches!(state.animation_state, AnimationState::Idle));

    // Should start at valid position within screen bounds
    assert!(state.position.0 >= 0);
    assert!(state.position.0 <= screen_width - sprite_size);
    assert!(state.position.1 >= 0);
    assert!(state.position.1 <= screen_height - sprite_size);

    // Should start at frame 0
    assert_eq!(state.current_frame, 0);
}

#[test]
fn test_update_animation_frame_cycles_frames() {
    let mut state = CompanionState::new(1920, 1080, 128);
    state.current_frame = 0;
    state.frame_timer = std::time::Instant::now() - Duration::from_millis(150);

    let frame_count = 4; // 4 frames in walk cycle

    state.update_animation_frame(frame_count);

    // Should have advanced to next frame
    assert_eq!(state.current_frame, 1);
}

#[test]
fn test_update_animation_frame_wraps_around() {
    let mut state = CompanionState::new(1920, 1080, 128);
    state.current_frame = 3;
    state.frame_timer = std::time::Instant::now() - Duration::from_millis(150);

    let frame_count = 4;

    state.update_animation_frame(frame_count);

    // Should wrap back to 0
    assert_eq!(state.current_frame, 0);
}

#[test]
fn test_update_animation_frame_respects_timing() {
    let mut state = CompanionState::new(1920, 1080, 128);
    state.current_frame = 0;
    state.frame_timer = std::time::Instant::now() - Duration::from_millis(50);

    let frame_count = 4;

    state.update_animation_frame(frame_count);

    // Should not advance (not enough time passed)
    assert_eq!(state.current_frame, 0);
}
