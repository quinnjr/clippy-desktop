use clippy_app::agent::{Agent, GREETING, IDLE_PREFIX, NEVER_RANDOM};
use clippy_app::companion::{Companion, EXIT_GRACE, REST_MS};
use std::time::{Duration, Instant};

fn companion(seed: u64) -> (Companion, Instant) {
    let t0 = Instant::now();
    (Companion::with_seed(Agent::clippy(), t0, seed), t0)
}

/// Advances deadline by deadline until `done` holds, returning the time reached.
fn run_until(
    companion: &mut Companion,
    mut now: Instant,
    max_steps: usize,
    done: impl Fn(&Companion) -> bool,
) -> Instant {
    for _ in 0..max_steps {
        if done(companion) {
            return now;
        }
        let deadline = companion
            .next_deadline()
            .expect("something is always scheduled");
        now = now.max(deadline);
        companion.advance(now);
    }
    panic!("condition not reached within {max_steps} steps");
}

fn is_idle(name: &str) -> bool {
    name.starts_with(IDLE_PREFIX)
}

#[test]
fn clippy_greets_first() {
    let (companion, _) = companion(1);
    assert_eq!(companion.animation(), Some(GREETING));
    assert_eq!(companion.frame_key(), Some((GREETING.to_string(), 0)));
    assert!(companion.frame().is_some());
    assert!(!companion.is_resting());
}

#[test]
fn after_the_greeting_clippy_rests_then_fidgets() {
    let (mut companion, t0) = companion(2);
    let rested_at = run_until(&mut companion, t0, 10_000, |c| c.is_resting());
    // Resting keeps the last greeting frame on screen and schedules the next move.
    assert_eq!(companion.animation(), Some(GREETING));
    let rest_end = companion.next_deadline().unwrap();
    let rest = rest_end.duration_since(rested_at);
    assert!(rest >= Duration::from_millis(*REST_MS.start()));
    assert!(rest <= Duration::from_millis(*REST_MS.end()));
    assert!(!companion.advance(rest_end - Duration::from_millis(1)));
    assert!(companion.is_resting());

    run_until(&mut companion, rested_at, 10_000, |c| !c.is_resting());
    assert!(
        is_idle(companion.animation().unwrap()),
        "{:?}",
        companion.animation()
    );
    assert_eq!(companion.frame_key().unwrap().1, 0);
}

#[test]
fn a_poke_while_resting_starts_a_gesture_at_once() {
    let (mut companion, t0) = companion(3);
    let now = run_until(&mut companion, t0, 10_000, |c| c.is_resting());
    let gestures = companion.agent().gesture_names();
    companion.poke(now);
    let name = companion.animation().unwrap().to_string();
    assert!(gestures.contains(&name), "{name} is not a gesture");
    assert_eq!(companion.frame_key(), Some((name, 0)));
    assert!(!companion.is_resting());
}

#[test]
fn a_poke_during_an_idle_animation_gets_a_gesture_soon() {
    let (mut companion, t0) = companion(4);
    let now = run_until(&mut companion, t0, 10_000, |c| {
        c.animation().is_some_and(is_idle)
    });
    let now = run_until(&mut companion, now, 100, |c| c.frame_key().unwrap().1 >= 3);
    companion.poke(now);
    let gestures = companion.agent().gesture_names();
    let started = run_until(&mut companion, now, 10_000, |c| {
        gestures.contains(&c.animation().unwrap().to_string())
    });
    assert!(started.duration_since(now) <= EXIT_GRACE + Duration::from_millis(2500));
}

#[test]
fn repeated_pokes_queue_at_most_one_gesture() {
    let (mut companion, t0) = companion(5);
    let now = run_until(&mut companion, t0, 10_000, |c| c.is_resting());
    companion.poke(now);
    let first = companion.animation().unwrap().to_string();
    companion.poke(now + Duration::from_millis(10));
    companion.poke(now + Duration::from_millis(20));
    companion.poke(now + Duration::from_millis(30));
    assert_eq!(
        companion.animation(),
        Some(first.as_str()),
        "pokes do not interrupt a gesture"
    );
    let mut names = Vec::new();
    let mut last = None;
    let mut clock = now;
    for _ in 0..20_000 {
        if companion.is_resting() && !names.is_empty() {
            break;
        }
        let name = companion.animation().unwrap().to_string();
        if last.as_deref() != Some(name.as_str()) {
            names.push(name.clone());
            last = Some(name);
        }
        clock = clock.max(companion.next_deadline().unwrap());
        companion.advance(clock);
    }
    assert_eq!(
        names.len(),
        2,
        "one gesture then exactly one queued gesture: {names:?}"
    );
}

#[test]
fn a_long_session_never_plays_forbidden_animations_and_never_repeats_an_idle() {
    let (mut companion, t0) = companion(6);
    let mut now = t0;
    let end = t0 + Duration::from_secs(45 * 60);
    let mut seen = Vec::new();
    let mut idles = Vec::new();
    let mut next_poke = t0 + Duration::from_secs(7);
    while now < end {
        assert!(companion.frame().is_some());
        let name = companion.animation().unwrap().to_string();
        if seen.last() != Some(&name) {
            if is_idle(&name) {
                idles.push(name.clone());
            }
            seen.push(name);
        }
        let deadline = companion.next_deadline().unwrap();
        if next_poke < deadline {
            now = now.max(next_poke);
            companion.poke(now);
            next_poke = now + Duration::from_secs(7);
        } else {
            now = now.max(deadline);
            companion.advance(now);
        }
    }
    for name in &seen {
        assert!(!NEVER_RANDOM.contains(&name.as_str()), "{name} was played");
    }
    for pair in idles.windows(2) {
        assert_ne!(pair[0], pair[1], "idle animation repeated back to back");
    }
    let distinct: std::collections::BTreeSet<&String> = seen.iter().collect();
    assert!(
        distinct.len() > 20,
        "only {} distinct animations in 45 minutes",
        distinct.len()
    );
    assert!(
        idles.len() > 50,
        "only {} idle animations in 45 minutes",
        idles.len()
    );
}

#[test]
fn frames_are_reported_as_changed_only_when_they_change() {
    let (mut companion, t0) = companion(8);
    assert!(!companion.advance(t0));
    let deadline = companion.next_deadline().unwrap();
    assert!(!companion.advance(deadline - Duration::from_millis(1)));
    assert!(companion.advance(deadline));
}
