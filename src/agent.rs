//! Animation data for an Office Assistant "agent".
//!
//! The data model mirrors the tables extracted from the original Microsoft
//! Agent character files (`CLIPPIT.ACS`) by the clippy.js project: a sprite
//! sheet plus named animations, each a list of frames that reference cells of
//! the sheet. Frames carry an explicit duration, optional random branching
//! (used to make idle animations loop for a while) and an optional exit
//! branch (where to jump when the animation is asked to wrap up early).

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;

/// Animation definitions for Clippy, extracted from the original Office
/// Assistant by the clippy.js project.
pub const CLIPPY_AGENT_JSON: &str = include_str!("../assets/clippy/agent.json");

/// Name of the animation played when Clippy first appears.
pub const GREETING: &str = "Greeting";

/// Animations that never get picked at random: they make Clippy disappear
/// (`Hide`, `GoodBye`), reappear from nothing (`Show`) or do nothing (`RestPose`).
pub const NEVER_RANDOM: &[&str] = &["Hide", "Show", "GoodBye", "RestPose"];

/// Prefix shared by the animations Clippy plays on his own while idle.
pub const IDLE_PREFIX: &str = "Idle";

#[derive(Debug, Clone, Deserialize)]
pub struct Agent {
    /// How many sprite-sheet cells a single frame may stack (Clippy uses 1).
    #[serde(rename = "overlayCount")]
    pub overlay_count: u32,
    /// Width and height of one frame, in sheet pixels.
    pub framesize: [u32; 2],
    /// All animations by name.
    pub animations: BTreeMap<String, Animation>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Animation {
    pub frames: Vec<Frame>,
    /// When set, the last frame is held until the animation is told to exit.
    #[serde(rename = "useExitBranching", default)]
    pub use_exit_branching: bool,
}

impl Animation {
    /// One nominal pass through the animation: the sum of its frame durations.
    /// Looping animations reach this only if they never branch backwards, so
    /// it doubles as a sensible "play for about this long" lifetime.
    pub fn total_duration(&self) -> std::time::Duration {
        self.frames
            .iter()
            .map(|frame| std::time::Duration::from_millis(frame.duration))
            .sum()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Frame {
    /// How long the frame stays on screen, in milliseconds.
    pub duration: u64,
    /// Sprite-sheet cells (x, y of the top-left corner) stacked bottom first.
    /// An empty list draws nothing, which is how Clippy hides.
    #[serde(default)]
    pub images: Vec<[u32; 2]>,
    /// Frame to jump to when the animation is asked to end early.
    #[serde(rename = "exitBranch")]
    pub exit_branch: Option<usize>,
    /// Random jumps evaluated when leaving this frame.
    pub branching: Option<Branching>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Branching {
    pub branches: Vec<Branch>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Branch {
    #[serde(rename = "frameIndex")]
    pub frame_index: usize,
    /// Probability of taking this branch, in percentage points.
    pub weight: f64,
}

/// A structural problem in agent data (dangling frame index, cell outside the
/// sprite sheet, ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidAgent(pub String);

impl fmt::Display for InvalidAgent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid agent data: {}", self.0)
    }
}

impl std::error::Error for InvalidAgent {}

impl Agent {
    /// Parses agent data from its JSON form.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// The embedded Clippy agent.
    pub fn clippy() -> Self {
        Self::from_json(CLIPPY_AGENT_JSON).expect("embedded agent.json is valid JSON")
    }

    /// Width and height of one frame, in sheet pixels.
    pub fn frame_size(&self) -> (u32, u32) {
        (self.framesize[0], self.framesize[1])
    }

    pub fn animation(&self, name: &str) -> Option<&Animation> {
        self.animations.get(name)
    }

    /// Names of the animations Clippy plays on his own while idle.
    pub fn idle_names(&self) -> Vec<String> {
        self.animations
            .keys()
            .filter(|name| name.starts_with(IDLE_PREFIX))
            .cloned()
            .collect()
    }

    /// Names of the animations that may be played when the user pokes Clippy.
    pub fn gesture_names(&self) -> Vec<String> {
        self.animations
            .keys()
            .filter(|name| !name.starts_with(IDLE_PREFIX))
            .filter(|name| !NEVER_RANDOM.contains(&name.as_str()))
            .cloned()
            .collect()
    }

    /// Checks that every frame index and sprite-sheet cell the data refers to
    /// exists, so the animator can index without bounds checks failing later.
    pub fn validate(&self, sheet_width: u32, sheet_height: u32) -> Result<(), InvalidAgent> {
        let (fw, fh) = self.frame_size();
        if fw == 0 || fh == 0 {
            return Err(InvalidAgent(format!("frame size {fw}x{fh} is empty")));
        }
        if fw > sheet_width || fh > sheet_height {
            return Err(InvalidAgent(format!(
                "frame size {fw}x{fh} is larger than the {sheet_width}x{sheet_height} sheet"
            )));
        }
        if self.animations.is_empty() {
            return Err(InvalidAgent("no animations".to_string()));
        }
        for (name, animation) in &self.animations {
            let count = animation.frames.len();
            if count == 0 {
                return Err(InvalidAgent(format!("animation {name} has no frames")));
            }
            for (index, frame) in animation.frames.iter().enumerate() {
                for &[x, y] in &frame.images {
                    if x + fw > sheet_width || y + fh > sheet_height {
                        return Err(InvalidAgent(format!(
                            "animation {name} frame {index} cell ({x}, {y}) lies outside the sheet"
                        )));
                    }
                }
                if let Some(target) = frame.exit_branch {
                    if target >= count {
                        return Err(InvalidAgent(format!(
                            "animation {name} frame {index} exit branch {target} is out of range"
                        )));
                    }
                }
                if let Some(branching) = &frame.branching {
                    for branch in &branching.branches {
                        if branch.frame_index >= count {
                            return Err(InvalidAgent(format!(
                                "animation {name} frame {index} branches to {} which is out of range",
                                branch.frame_index
                            )));
                        }
                        if !branch.weight.is_finite() || branch.weight < 0.0 {
                            return Err(InvalidAgent(format!(
                                "animation {name} frame {index} has a bad branch weight {}",
                                branch.weight
                            )));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
