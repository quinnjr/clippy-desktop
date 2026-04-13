use clippy_app::companion::CompanionState;
use clippy_app::animation::AnimationState;

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
