use clippy_app::companion::CompanionState;
use clippy_app::animation::{AnimationState, Direction};
use std::time::Duration;

#[test]
fn test_update_behavior_starts_walking_from_idle() {
    let mut state = CompanionState::new(1920, 1080, 128);
    state.animation_state = AnimationState::Idle;

    // Force behavior update by setting timer to past
    state.behavior_timer = std::time::Instant::now() - Duration::from_secs(10);

    // Call update_behavior multiple times until walking starts (it's probabilistic)
    let mut found_walking = false;
    for _ in 0..100 {
        state.update_behavior();
        if matches!(state.animation_state, AnimationState::Walking { .. }) {
            found_walking = true;
            break;
        }
        // Reset timer for next attempt
        state.behavior_timer = std::time::Instant::now() - Duration::from_secs(10);
    }

    assert!(found_walking, "Should eventually transition to walking state");
}

#[test]
fn test_update_behavior_returns_to_idle_after_walk_duration() {
    let mut state = CompanionState::new(1920, 1080, 128);
    state.animation_state = AnimationState::Walking { direction: Direction::Left };
    state.walk_duration = Duration::from_millis(100);
    state.walk_started = std::time::Instant::now() - Duration::from_millis(200);

    state.update_behavior();

    // Should return to idle since walk duration expired
    assert!(matches!(state.animation_state, AnimationState::Idle));
}

#[test]
fn test_update_position_moves_when_walking() {
    let mut state = CompanionState::new(1920, 1080, 128);
    state.position = (500, 500);
    state.animation_state = AnimationState::Walking { direction: Direction::Left };

    let old_x = state.position.0;

    state.update_position(1920, 1080, 128);

    // Should have moved left (x decreased)
    assert!(state.position.0 < old_x);
}

#[test]
fn test_update_position_does_not_move_when_idle() {
    let mut state = CompanionState::new(1920, 1080, 128);
    state.position = (500, 500);
    state.animation_state = AnimationState::Idle;

    let old_pos = state.position;

    state.update_position(1920, 1080, 128);

    // Should not have moved
    assert_eq!(state.position, old_pos);
}
