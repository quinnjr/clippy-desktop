use clippy_app::sprite::SpriteSheet;

#[test]
fn test_spritesheet_loads_embedded_assets() {
    let sheet = SpriteSheet::load_embedded();

    assert!(!sheet.idle_frames.is_empty(), "Should have idle frames");
    assert!(!sheet.walk_left_frames.is_empty(), "Should have walk_left frames");
    assert!(!sheet.walk_right_frames.is_empty(), "Should have walk_right frames");

    // Verify frame dimensions (128x128)
    let first_idle = &sheet.idle_frames[0];
    assert_eq!(first_idle.width(), 128);
    assert_eq!(first_idle.height(), 128);
}

#[test]
fn test_spritesheet_get_frame() {
    let sheet = SpriteSheet::load_embedded();

    // Test idle frame access
    let idle_frame = sheet.get_frame(&clippy_app::animation::AnimationState::Idle, 0);
    assert!(idle_frame.is_some());

    // Test walking frame access
    let walk_frame = sheet.get_frame(
        &clippy_app::animation::AnimationState::Walking {
            direction: clippy_app::animation::Direction::Left
        },
        0
    );
    assert!(walk_frame.is_some());
}
