//! Frame-by-frame playback of one animation.
//!
//! This is a direct port of the stepping rules of the original Office
//! Assistant player (as reimplemented by clippy.js): each frame is shown for
//! its own duration, leaving a frame may take a weighted random branch, and
//! once an animation is asked to exit it follows the exit branches instead.
//! Time is passed in explicitly so the logic is deterministic under test.

use crate::agent::{Agent, Animation, Frame};
use rand::Rng;
use std::time::{Duration, Instant};

/// Guards against animation data that chains zero-duration frames forever.
const MAX_STEPS_PER_ADVANCE: usize = 256;

/// What happened during a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepEvent {
    /// The animation is still going.
    Continue,
    /// The animation just reached its final frame.
    Exited,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownAnimation(pub String);

impl std::fmt::Display for UnknownAnimation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown animation {:?}", self.0)
    }
}

impl std::error::Error for UnknownAnimation {}

#[derive(Debug, Clone, Default)]
pub struct Animator {
    animation: Option<String>,
    /// Frame the playback cursor is on.
    frame_index: usize,
    /// Frame currently on screen; `None` before the first step.
    displayed: Option<usize>,
    exiting: bool,
    next_step: Option<Instant>,
}

impl Animator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Name of the animation being played (or held on its last frame).
    pub fn animation(&self) -> Option<&str> {
        self.animation.as_deref()
    }

    /// Index of the frame on screen.
    pub fn displayed_index(&self) -> Option<usize> {
        self.displayed
    }

    /// The frame on screen.
    pub fn displayed_frame<'a>(&self, agent: &'a Agent) -> Option<&'a Frame> {
        let animation = agent.animation(self.animation.as_deref()?)?;
        animation.frames.get(self.displayed?)
    }

    /// When the next step is due; `None` while nothing is scheduled.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.next_step
    }

    /// Starts an animation from its first frame, showing that frame at once.
    pub fn play(
        &mut self,
        agent: &Agent,
        name: &str,
        now: Instant,
        rng: &mut impl Rng,
    ) -> Result<StepEvent, UnknownAnimation> {
        let animation = agent
            .animation(name)
            .ok_or_else(|| UnknownAnimation(name.to_string()))?;
        self.animation = Some(name.to_string());
        self.frame_index = 0;
        self.displayed = None;
        self.exiting = false;
        Ok(self.step(animation, now, rng))
    }

    /// Asks the animation to wrap up: from now on frames follow their exit
    /// branches, which lead to the final frame.
    pub fn exit(&mut self) {
        self.exiting = true;
    }

    /// Freezes playback on the frame currently shown.
    pub fn hold(&mut self) {
        self.next_step = None;
    }

    /// Whether the current frame has an exit branch to follow.
    pub fn can_exit_now(&self, agent: &Agent) -> bool {
        self.displayed_frame(agent)
            .map(|frame| frame.exit_branch.is_some())
            .unwrap_or(false)
    }

    /// Performs every step whose deadline has passed. Reports `Exited` if the
    /// animation reached its final frame during this call.
    pub fn advance(&mut self, agent: &Agent, now: Instant, rng: &mut impl Rng) -> StepEvent {
        let Some(animation) = self
            .animation
            .as_deref()
            .and_then(|name| agent.animation(name))
        else {
            return StepEvent::Continue;
        };
        let mut result = StepEvent::Continue;
        for _ in 0..MAX_STEPS_PER_ADVANCE {
            match self.next_step {
                Some(deadline) if deadline <= now => {
                    if self.step(animation, now, rng) == StepEvent::Exited {
                        result = StepEvent::Exited;
                    }
                }
                _ => break,
            }
        }
        result
    }

    fn step(&mut self, animation: &Animation, now: Instant, rng: &mut impl Rng) -> StepEvent {
        let last = animation.frames.len() - 1;
        let next = self.next_frame_index(animation, rng).min(last);
        let frame_changed = self.displayed.is_none() || self.frame_index != next;
        self.frame_index = next;
        let at_last = next == last;
        // Hold the previous frame at the end of an exit-branching animation
        // until it is told to exit.
        if !(at_last && animation.use_exit_branching) {
            self.displayed = Some(next);
        }
        let shown = self.displayed.unwrap_or(next);
        self.next_step = Some(now + Duration::from_millis(animation.frames[shown].duration));
        let finished = frame_changed && at_last && (!animation.use_exit_branching || self.exiting);
        if finished {
            StepEvent::Exited
        } else {
            StepEvent::Continue
        }
    }

    fn next_frame_index(&self, animation: &Animation, rng: &mut impl Rng) -> usize {
        let Some(current) = self.displayed else {
            return 0;
        };
        let frame = &animation.frames[current];
        if self.exiting {
            if let Some(target) = frame.exit_branch {
                return target;
            }
        }
        if let Some(branching) = &frame.branching {
            let mut roll: f64 = rng.gen_range(0.0..100.0);
            for branch in &branching.branches {
                if roll <= branch.weight {
                    return branch.frame_index;
                }
                roll -= branch.weight;
            }
        }
        self.frame_index + 1
    }
}
