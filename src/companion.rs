//! Clippy's behaviour: greet, rest, fidget through idle animations, and react
//! when poked. Time is passed in explicitly so the logic is testable.
//!
//! Several of the original animations loop forever until the host dismisses
//! them (`Writing`, `Searching`, the finger-tap fidget, ...). To play those to
//! a natural close, every animation is given a lifetime of one nominal pass;
//! once it elapses Clippy asks the animation to wrap up through its exit
//! branches, exactly the way the original Office Assistant ended a looping
//! animation.

use crate::agent::{Agent, Frame, GREETING};
use crate::animator::{Animator, StepEvent};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::time::{Duration, Instant};

/// How long Clippy sits still between idle animations, in milliseconds.
pub const REST_MS: std::ops::RangeInclusive<u64> = 2000..=6000;

/// Longest Clippy waits for an animation to wrap up after being asked to exit.
pub const EXIT_GRACE: Duration = Duration::from_millis(1500);

/// Shortest lifetime given to any animation, so nothing flickers past.
pub const MIN_LIFETIME: Duration = Duration::from_millis(600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Sitting still on the last frame of the previous animation.
    Resting { until: Instant },
    /// Playing one of the idle fidgets.
    Idle,
    /// Playing a gesture (the greeting or a reaction to a poke).
    Gesture,
    /// Wrapping up an animation so it (and any queued gesture) can end.
    Exiting { deadline: Instant },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pool {
    Idle,
    Gesture,
}

pub struct Companion {
    agent: Agent,
    animator: Animator,
    rng: StdRng,
    mode: Mode,
    /// When the current idle/gesture should be asked to wrap up.
    play_until: Instant,
    pending: Option<String>,
    last_idle: Option<String>,
    last_gesture: Option<String>,
    idle_names: Vec<String>,
    gesture_names: Vec<String>,
}

impl Companion {
    /// Creates Clippy and starts his greeting.
    pub fn new(agent: Agent, now: Instant) -> Self {
        Self::with_rng(agent, now, StdRng::from_entropy())
    }

    /// Deterministic variant for tests.
    pub fn with_seed(agent: Agent, now: Instant, seed: u64) -> Self {
        Self::with_rng(agent, now, StdRng::seed_from_u64(seed))
    }

    fn with_rng(agent: Agent, now: Instant, rng: StdRng) -> Self {
        let idle_names = agent.idle_names();
        let gesture_names = agent.gesture_names();
        let mut companion = Self {
            agent,
            animator: Animator::new(),
            rng,
            mode: Mode::Resting { until: now },
            play_until: now,
            pending: None,
            last_idle: None,
            last_gesture: None,
            idle_names,
            gesture_names,
        };
        if companion.agent.animation(GREETING).is_some() {
            companion.play(GREETING, Mode::Gesture, now);
        }
        companion
    }

    pub fn agent(&self) -> &Agent {
        &self.agent
    }

    /// The frame to draw right now (`None` only if nothing has ever played).
    pub fn frame(&self) -> Option<&Frame> {
        self.animator.displayed_frame(&self.agent)
    }

    /// Name of the animation on screen.
    pub fn animation(&self) -> Option<&str> {
        self.animator.animation()
    }

    /// Whether Clippy is sitting still between animations.
    pub fn is_resting(&self) -> bool {
        matches!(self.mode, Mode::Resting { .. })
    }

    /// Identifies the frame on screen, for change detection.
    pub fn frame_key(&self) -> Option<(String, usize)> {
        Some((
            self.animator.animation()?.to_string(),
            self.animator.displayed_index()?,
        ))
    }

    /// When `advance` next needs to run.
    pub fn next_deadline(&self) -> Option<Instant> {
        match self.mode {
            Mode::Resting { until } => Some(until),
            Mode::Idle | Mode::Gesture => Some(match self.animator.next_deadline() {
                Some(step) => step.min(self.play_until),
                None => self.play_until,
            }),
            Mode::Exiting { deadline } => Some(match self.animator.next_deadline() {
                Some(step) => step.min(deadline),
                None => deadline,
            }),
        }
    }

    /// Moves time forward. Returns `true` when the frame on screen changed.
    pub fn advance(&mut self, now: Instant) -> bool {
        let before = self.frame_key();
        // A running animation that has outstayed its lifetime is asked to wrap
        // up; from here the shared Exiting path drives it to its close.
        if matches!(self.mode, Mode::Idle | Mode::Gesture) && now >= self.play_until {
            self.animator.exit();
            self.mode = Mode::Exiting {
                deadline: now + EXIT_GRACE,
            };
        }
        match self.mode {
            Mode::Resting { until } => {
                if now >= until {
                    self.play_idle(now);
                }
            }
            Mode::Idle | Mode::Gesture => {
                if self.animator.advance(&self.agent, now, &mut self.rng) == StepEvent::Exited {
                    self.finish(now);
                }
            }
            Mode::Exiting { deadline } => {
                let event = self.animator.advance(&self.agent, now, &mut self.rng);
                if event == StepEvent::Exited || now >= deadline {
                    self.finish(now);
                }
            }
        }
        self.frame_key() != before
    }

    /// The user clicked Clippy: play a gesture as soon as it makes sense.
    pub fn poke(&mut self, now: Instant) {
        let Some(gesture) = self.pick(Pool::Gesture) else {
            return;
        };
        match self.mode {
            Mode::Resting { .. } => {
                self.last_gesture = Some(gesture.clone());
                self.play(&gesture, Mode::Gesture, now);
            }
            Mode::Idle => {
                self.pending = Some(gesture);
                self.animator.exit();
                self.mode = Mode::Exiting {
                    deadline: now + EXIT_GRACE,
                };
            }
            Mode::Gesture | Mode::Exiting { .. } => {
                if self.pending.is_none() {
                    self.pending = Some(gesture);
                }
            }
        }
    }

    /// The current animation ended: play what is queued, or rest.
    fn finish(&mut self, now: Instant) {
        match self.pending.take() {
            Some(gesture) => {
                self.last_gesture = Some(gesture.clone());
                self.play(&gesture, Mode::Gesture, now);
            }
            None => self.rest(now),
        }
    }

    fn rest(&mut self, now: Instant) {
        self.animator.hold();
        let millis = self.rng.gen_range(REST_MS);
        self.mode = Mode::Resting {
            until: now + Duration::from_millis(millis),
        };
    }

    fn play_idle(&mut self, now: Instant) {
        match self.pick(Pool::Idle) {
            Some(name) => {
                self.last_idle = Some(name.clone());
                self.play(&name, Mode::Idle, now);
            }
            None => self.rest(now),
        }
    }

    fn play(&mut self, name: &str, mode: Mode, now: Instant) {
        match self.animator.play(&self.agent, name, now, &mut self.rng) {
            Ok(StepEvent::Continue) => {
                self.play_until = now + self.lifetime(name);
                self.mode = mode;
            }
            // A one-frame animation is over as soon as it starts.
            Ok(StepEvent::Exited) => self.rest(now),
            Err(_) => self.rest(now),
        }
    }

    /// One nominal pass through the animation, with a small floor.
    fn lifetime(&self, name: &str) -> Duration {
        self.agent
            .animation(name)
            .map(|animation| animation.total_duration().max(MIN_LIFETIME))
            .unwrap_or(MIN_LIFETIME)
    }

    /// Picks a random name from a pool, avoiding an immediate repeat when
    /// the pool offers a choice.
    fn pick(&mut self, pool: Pool) -> Option<String> {
        let (names, avoid) = match pool {
            Pool::Idle => (&self.idle_names, self.last_idle.as_deref()),
            Pool::Gesture => (&self.gesture_names, self.last_gesture.as_deref()),
        };
        if names.is_empty() {
            return None;
        }
        let candidates: Vec<&str> = names
            .iter()
            .map(String::as_str)
            .filter(|name| Some(*name) != avoid)
            .collect();
        let chosen = if candidates.is_empty() {
            names[0].as_str()
        } else {
            candidates[self.rng.gen_range(0..candidates.len())]
        };
        Some(chosen.to_string())
    }
}
