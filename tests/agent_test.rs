use clippy_app::agent::{Agent, GREETING, IDLE_PREFIX, NEVER_RANDOM};
use clippy_app::sprite::SpriteSheet;

#[test]
fn embedded_clippy_data_is_valid_against_its_sprite_sheet() {
    let agent = Agent::clippy();
    let sheet = SpriteSheet::clippy();
    agent
        .validate(sheet.width(), sheet.height())
        .expect("embedded agent data references only existing sheet cells and frames");
    assert_eq!(agent.frame_size(), (124, 93));
    assert_eq!(agent.overlay_count, 1);
    assert_eq!(agent.animations.len(), 43);
    assert!(agent.animation(GREETING).is_some());
}

#[test]
fn idle_and_gesture_pools_are_disjoint_and_exclude_vanishing_animations() {
    let agent = Agent::clippy();
    let idle = agent.idle_names();
    let gestures = agent.gesture_names();
    assert_eq!(idle.len(), 8);
    assert!(idle.iter().all(|name| name.starts_with(IDLE_PREFIX)));
    assert!(!gestures.is_empty());
    for name in &gestures {
        assert!(
            !name.starts_with(IDLE_PREFIX),
            "{name} is an idle animation"
        );
        assert!(
            !NEVER_RANDOM.contains(&name.as_str()),
            "{name} must never be picked at random"
        );
        assert!(agent.animation(name).is_some());
    }
    for name in NEVER_RANDOM {
        assert!(agent.animation(name).is_some(), "{name} exists in the data");
    }
    assert!(gestures.iter().any(|name| name == "Wave"));
    assert!(gestures.iter().any(|name| name == "Congratulate"));
}

#[test]
fn every_frame_is_drawable_or_deliberately_blank() {
    let agent = Agent::clippy();
    let mut blank = 0;
    for animation in agent.animations.values() {
        for frame in &animation.frames {
            if frame.images.is_empty() {
                blank += 1;
            }
            assert!(frame.images.len() <= agent.overlay_count as usize);
        }
    }
    // Only the hide/show transitions draw nothing.
    assert!(
        blank > 0 && blank < 10,
        "unexpected number of blank frames: {blank}"
    );
}

#[test]
fn zero_duration_frames_cannot_spin_forever() {
    // A zero-duration frame that branches to itself is re-rolled at once; the
    // original player relies on the branch failing eventually, so its weight
    // must leave room for that.
    let agent = Agent::clippy();
    let mut self_loops = 0;
    for (name, animation) in &agent.animations {
        for (index, frame) in animation.frames.iter().enumerate() {
            if frame.duration != 0 {
                continue;
            }
            if let Some(branching) = &frame.branching {
                for branch in &branching.branches {
                    if branch.frame_index == index {
                        self_loops += 1;
                        assert!(branch.weight < 100.0, "{name} frame {index} spins forever");
                    }
                }
            }
        }
    }
    assert!(self_loops > 0, "the data is known to contain such a frame");
}

fn synthetic(animations: &str) -> Agent {
    Agent::from_json(&format!(
        r#"{{"overlayCount":1,"framesize":[2,2],"animations":{{{animations}}}}}"#
    ))
    .expect("synthetic agent parses")
}

#[test]
fn validate_rejects_cells_outside_the_sheet() {
    let agent = synthetic(r#""A":{"frames":[{"duration":10,"images":[[5,0]]}]}"#);
    let err = agent.validate(6, 2).unwrap_err();
    assert!(err.0.contains("outside the sheet"), "{err}");
    assert!(agent.validate(7, 2).is_ok());
}

#[test]
fn validate_rejects_dangling_frame_indices() {
    let agent = synthetic(r#""A":{"frames":[{"duration":10,"images":[[0,0]],"exitBranch":3}]}"#);
    assert!(agent.validate(2, 2).unwrap_err().0.contains("exit branch"));
    let agent = synthetic(
        r#""A":{"frames":[{"duration":10,"images":[[0,0]],"branching":{"branches":[{"frameIndex":9,"weight":50}]}}]}"#,
    );
    assert!(agent
        .validate(2, 2)
        .unwrap_err()
        .0
        .contains("branches to 9"));
}

#[test]
fn validate_rejects_empty_animations_and_bad_weights() {
    let agent = synthetic(r#""A":{"frames":[]}"#);
    assert!(agent.validate(2, 2).unwrap_err().0.contains("no frames"));
    let agent = synthetic(
        r#""A":{"frames":[{"duration":10,"images":[[0,0]],"branching":{"branches":[{"frameIndex":0,"weight":-1}]}}]}"#,
    );
    assert!(agent
        .validate(2, 2)
        .unwrap_err()
        .0
        .contains("bad branch weight"));
}

#[test]
fn unknown_json_fields_are_ignored() {
    let agent = synthetic(r#""A":{"frames":[{"duration":10,"images":[[0,0]],"sound":"1"}]}"#);
    assert!(agent.validate(2, 2).is_ok());
}
