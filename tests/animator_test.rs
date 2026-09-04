use clippy_app::agent::Agent;
use clippy_app::animator::{Animator, StepEvent};
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::time::{Duration, Instant};

const SYNTHETIC: &str = r#"{"overlayCount":1,"framesize":[2,2],"animations":{
  "Linear":{"frames":[{"duration":100,"images":[[0,0]]},{"duration":100,"images":[[2,0]]},{"duration":100,"images":[[4,0]]}]},
  "Loop":{"frames":[{"duration":100,"images":[[0,0]],"branching":{"branches":[{"frameIndex":0,"weight":100}]}},{"duration":100,"images":[[2,0]]}]},
  "Exit":{"frames":[{"duration":100,"images":[[0,0]],"exitBranch":2,"branching":{"branches":[{"frameIndex":0,"weight":100}]}},{"duration":100,"images":[[2,0]]},{"duration":100,"images":[[4,0]]}]},
  "Held":{"useExitBranching":true,"frames":[{"duration":100,"images":[[0,0]]},{"duration":100,"images":[[2,0]],"exitBranch":3},{"duration":100,"images":[[4,0]],"exitBranch":1},{"duration":100,"images":[[0,0]]}]},
  "Zero":{"frames":[{"duration":0,"images":[[0,0]]},{"duration":0,"images":[[2,0]]},{"duration":100,"images":[[4,0]]}]},
  "Single":{"frames":[{"duration":100,"images":[[0,0]]}]},
  "Blank":{"frames":[{"duration":100},{"duration":100,"images":[[0,0]]}]},
  "Coin":{"frames":[{"duration":100,"images":[[0,0]],"branching":{"branches":[{"frameIndex":2,"weight":50}]}},{"duration":100,"images":[[2,0]]},{"duration":100,"images":[[4,0]]}]}
}}"#;

fn agent() -> Agent {
    let agent = Agent::from_json(SYNTHETIC).expect("synthetic data parses");
    agent.validate(6, 2).expect("synthetic data is valid");
    agent
}

fn rng() -> StdRng {
    StdRng::seed_from_u64(7)
}

fn ms(t0: Instant, millis: u64) -> Instant {
    t0 + Duration::from_millis(millis)
}

#[test]
fn plays_frames_in_order_and_reports_the_end_once() {
    let agent = agent();
    let mut rng = rng();
    let mut animator = Animator::new();
    let t0 = Instant::now();

    assert_eq!(
        animator.play(&agent, "Linear", t0, &mut rng),
        Ok(StepEvent::Continue)
    );
    assert_eq!(animator.animation(), Some("Linear"));
    assert_eq!(animator.displayed_index(), Some(0));
    assert_eq!(animator.next_deadline(), Some(ms(t0, 100)));

    assert_eq!(
        animator.advance(&agent, ms(t0, 50), &mut rng),
        StepEvent::Continue
    );
    assert_eq!(animator.displayed_index(), Some(0));

    assert_eq!(
        animator.advance(&agent, ms(t0, 100), &mut rng),
        StepEvent::Continue
    );
    assert_eq!(animator.displayed_index(), Some(1));

    assert_eq!(
        animator.advance(&agent, ms(t0, 200), &mut rng),
        StepEvent::Exited
    );
    assert_eq!(animator.displayed_index(), Some(2));

    // Holding on the last frame does not report the end again.
    assert_eq!(
        animator.advance(&agent, ms(t0, 300), &mut rng),
        StepEvent::Continue
    );
    assert_eq!(
        animator.advance(&agent, ms(t0, 400), &mut rng),
        StepEvent::Continue
    );
    assert_eq!(animator.displayed_index(), Some(2));
}

#[test]
fn a_late_advance_moves_one_frame_and_reschedules_from_now() {
    // After a long stall (a suspended laptop, say) Clippy resumes where he
    // was instead of bursting through the frames he missed.
    let agent = agent();
    let mut rng = rng();
    let mut animator = Animator::new();
    let t0 = Instant::now();
    animator.play(&agent, "Linear", t0, &mut rng).unwrap();
    assert_eq!(
        animator.advance(&agent, ms(t0, 1000), &mut rng),
        StepEvent::Continue
    );
    assert_eq!(animator.displayed_index(), Some(1));
    assert_eq!(animator.next_deadline(), Some(ms(t0, 1100)));
}

#[test]
fn certain_branches_are_always_taken() {
    let agent = agent();
    let mut rng = rng();
    let mut animator = Animator::new();
    let t0 = Instant::now();
    animator.play(&agent, "Loop", t0, &mut rng).unwrap();
    for step in 1..=20 {
        assert_eq!(
            animator.advance(&agent, ms(t0, step * 100), &mut rng),
            StepEvent::Continue
        );
        assert_eq!(animator.displayed_index(), Some(0), "step {step}");
    }
}

#[test]
fn a_fifty_percent_branch_is_taken_about_half_the_time() {
    let agent = agent();
    let mut rng = rng();
    let mut jumped = 0;
    let trials = 400;
    for _ in 0..trials {
        let mut animator = Animator::new();
        let t0 = Instant::now();
        animator.play(&agent, "Coin", t0, &mut rng).unwrap();
        animator.advance(&agent, ms(t0, 100), &mut rng);
        if animator.displayed_index() == Some(2) {
            jumped += 1;
        } else {
            assert_eq!(animator.displayed_index(), Some(1));
        }
    }
    assert!(
        (120..=280).contains(&jumped),
        "took the branch {jumped} times out of {trials}"
    );
}

#[test]
fn exiting_follows_exit_branches_out_of_a_loop() {
    let agent = agent();
    let mut rng = rng();
    let mut animator = Animator::new();
    let t0 = Instant::now();
    animator.play(&agent, "Exit", t0, &mut rng).unwrap();
    for step in 1..=5 {
        animator.advance(&agent, ms(t0, step * 100), &mut rng);
        assert_eq!(animator.displayed_index(), Some(0));
    }
    assert!(animator.can_exit_now(&agent));
    animator.exit();
    assert_eq!(
        animator.advance(&agent, ms(t0, 600), &mut rng),
        StepEvent::Exited
    );
    assert_eq!(animator.displayed_index(), Some(2));
}

#[test]
fn exit_branching_animations_hold_their_waiting_frame_until_told_to_exit() {
    let agent = agent();
    let mut rng = rng();
    let mut animator = Animator::new();
    let t0 = Instant::now();
    animator.play(&agent, "Held", t0, &mut rng).unwrap();
    assert_eq!(
        animator.advance(&agent, ms(t0, 100), &mut rng),
        StepEvent::Continue
    );
    assert_eq!(animator.displayed_index(), Some(1));
    assert_eq!(
        animator.advance(&agent, ms(t0, 200), &mut rng),
        StepEvent::Continue
    );
    assert_eq!(animator.displayed_index(), Some(2));
    // The cursor reaches the last frame, but the waiting frame stays on screen.
    for step in 3..=6 {
        assert_eq!(
            animator.advance(&agent, ms(t0, step * 100), &mut rng),
            StepEvent::Continue
        );
        assert_eq!(animator.displayed_index(), Some(2), "step {step}");
    }
    assert!(animator.can_exit_now(&agent));
    animator.exit();
    // The waiting frame's exit branch leads to the wrap-up frame...
    assert_eq!(
        animator.advance(&agent, ms(t0, 700), &mut rng),
        StepEvent::Continue
    );
    assert_eq!(animator.displayed_index(), Some(1));
    // ...whose own exit branch reaches the final frame, ending the animation.
    assert_eq!(
        animator.advance(&agent, ms(t0, 800), &mut rng),
        StepEvent::Exited
    );
    assert_eq!(animator.displayed_index(), Some(1));
}

#[test]
fn zero_duration_frames_pass_within_one_advance() {
    let agent = agent();
    let mut rng = rng();
    let mut animator = Animator::new();
    let t0 = Instant::now();
    assert_eq!(
        animator.play(&agent, "Zero", t0, &mut rng),
        Ok(StepEvent::Continue)
    );
    assert_eq!(animator.next_deadline(), Some(t0));
    assert_eq!(animator.advance(&agent, t0, &mut rng), StepEvent::Exited);
    assert_eq!(animator.displayed_index(), Some(2));
    assert_eq!(animator.next_deadline(), Some(ms(t0, 100)));
}

#[test]
fn a_single_frame_animation_ends_as_it_starts() {
    let agent = agent();
    let mut rng = rng();
    let mut animator = Animator::new();
    let t0 = Instant::now();
    assert_eq!(
        animator.play(&agent, "Single", t0, &mut rng),
        Ok(StepEvent::Exited)
    );
    assert_eq!(animator.displayed_index(), Some(0));
}

#[test]
fn blank_frames_have_no_cells() {
    let agent = agent();
    let mut rng = rng();
    let mut animator = Animator::new();
    let t0 = Instant::now();
    animator.play(&agent, "Blank", t0, &mut rng).unwrap();
    assert!(animator.displayed_frame(&agent).unwrap().images.is_empty());
    animator.advance(&agent, ms(t0, 100), &mut rng);
    assert_eq!(
        animator.displayed_frame(&agent).unwrap().images,
        vec![[0, 0]]
    );
}

#[test]
fn hold_freezes_the_frame_and_clears_the_deadline() {
    let agent = agent();
    let mut rng = rng();
    let mut animator = Animator::new();
    let t0 = Instant::now();
    animator.play(&agent, "Linear", t0, &mut rng).unwrap();
    animator.hold();
    assert_eq!(animator.next_deadline(), None);
    assert_eq!(
        animator.advance(&agent, ms(t0, 10_000), &mut rng),
        StepEvent::Continue
    );
    assert_eq!(animator.displayed_index(), Some(0));
    assert_eq!(animator.animation(), Some("Linear"));
}

#[test]
fn unknown_animations_are_rejected_without_changing_state() {
    let agent = agent();
    let mut rng = rng();
    let mut animator = Animator::new();
    let t0 = Instant::now();
    animator.play(&agent, "Linear", t0, &mut rng).unwrap();
    assert!(animator.play(&agent, "Nope", t0, &mut rng).is_err());
    assert_eq!(animator.animation(), Some("Linear"));
    assert_eq!(animator.displayed_index(), Some(0));
}
