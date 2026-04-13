use clippy_app::animation::{AnimationState, Direction};

#[test]
fn test_direction_has_left_and_right() {
    let left = Direction::Left;
    let right = Direction::Right;

    // Verify they're different
    assert!(matches!(left, Direction::Left));
    assert!(matches!(right, Direction::Right));
}

#[test]
fn test_animation_state_variants() {
    let idle = AnimationState::Idle;
    let walking = AnimationState::Walking { direction: Direction::Left };

    assert!(matches!(idle, AnimationState::Idle));
    assert!(matches!(walking, AnimationState::Walking { .. }));

    if let AnimationState::Walking { direction } = walking {
        assert_eq!(direction, Direction::Left);
    } else {
        panic!("Expected Walking variant");
    }
}
