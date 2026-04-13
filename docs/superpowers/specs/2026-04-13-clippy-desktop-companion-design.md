# Clippy Desktop Companion - Design Specification

**Date:** 2026-04-13  
**Status:** Approved

## Overview

A cross-platform desktop companion application featuring Microsoft Clippy that wanders around the screen with random walk behavior and idle animations. The app is purely decorative with no interactive functionality beyond a right-click quit menu.

## Goals

- Create a lightweight, native desktop overlay application
- Display Clippy sprite animations (idle and walking)
- Implement random wandering behavior across the screen
- Support Windows, macOS, and Linux (including Wayland)
- Keep resource usage minimal

## Non-Goals

- No interactive features (tooltips, clipboard monitoring, tips)
- No configuration UI or settings file
- No system tray icon
- No click-through window behavior

## Architecture

### High-Level Structure

```
┌─────────────────────────────────────┐
│       Main Event Loop               │
│  (winit ApplicationHandler)         │
├─────────────────────────────────────┤
│  ┌─────────────┐  ┌──────────────┐ │
│  │  Animation  │  │   Movement   │ │
│  │    State    │  │    Engine    │ │
│  │   Machine   │  │              │ │
│  └─────────────┘  └──────────────┘ │
├─────────────────────────────────────┤
│  ┌─────────────┐  ┌──────────────┐ │
│  │   Sprite    │  │   Renderer   │ │
│  │   Sheet     │  │ (softbuffer) │ │
│  └─────────────┘  └──────────────┘ │
└─────────────────────────────────────┘
         │                    │
         ▼                    ▼
    Asset Files      Transparent Window
```

The application consists of five core layers:

1. **Window Layer** - Manages the frameless, transparent OS window via `winit`
2. **Rendering Layer** - CPU-based pixel rendering using `softbuffer`
3. **Animation State Machine** - Tracks and transitions between idle/walking states
4. **Movement Engine** - Calculates position updates and screen boundary handling
5. **Asset Manager** - Loads and stores Clippy sprite frames

### Event Loop Flow

The main loop processes:
- Window events (resize, close, mouse clicks)
- Timer events for animation frame updates (~100ms intervals)
- Timer events for behavior state changes (2-5 second intervals)
- Rendering updates (60 FPS target)

## Core Components

### Data Structures

**AnimationState**
```rust
enum AnimationState {
    Idle,
    Walking { direction: Direction },
}
```

**Direction**
```rust
enum Direction {
    Left,
    Right,
}
```
Note: Initial implementation focuses on horizontal movement only. Vertical and diagonal directions can be added in future iterations.

**SpriteSheet**
```rust
struct SpriteSheet {
    idle_frames: Vec<RgbaImage>,
    walk_left_frames: Vec<RgbaImage>,
    walk_right_frames: Vec<RgbaImage>,
}
```
Holds decoded sprite frame data. Loaded once at startup from embedded assets.

**CompanionState**
```rust
struct CompanionState {
    position: (i32, i32),           // Screen coordinates
    animation_state: AnimationState,
    current_frame: usize,
    frame_timer: Instant,           // Last frame update time
    behavior_timer: Instant,        // Last behavior change time
    walk_duration: Duration,        // How long to walk in current direction
    walk_started: Instant,          // When current walk began
}
```

### Behavior Logic

**Random Walk Algorithm:**
1. Every 2-5 seconds (random interval), evaluate current state:
   - If idle: 60% chance start walking, 40% chance stay idle
   - If walking: check if walk duration expired, return to idle
2. When starting a walk:
   - Choose random direction (left or right)
   - Choose random walk duration (1-3 seconds)
   - Transition to Walking state
3. While walking:
   - Move 2-5 pixels per frame at 60 FPS
   - Update window position via `set_outer_position()`
   - Check screen boundaries - if hit edge, end walk early

**Animation Frame Updates:**
- Independent from position updates
- Walking animation: 4-8 frames cycling at ~10 FPS (100ms per frame)
- Idle animation: slower cycle or static with occasional variation
- Frame index wraps around when reaching end of frame array

**Screen Boundary Handling:**
- Query primary monitor dimensions using `winit::monitor::MonitorHandle`
- Clamp position to: `0 ≤ x ≤ screen_width - sprite_width`, `0 ≤ y ≤ screen_height - sprite_height`
- When boundary hit during walk, immediately end walk and return to idle

## Cross-Platform Implementation

### Window Creation

Use `winit::window::WindowBuilder` with:
- `.with_decorations(false)` - Remove title bar and borders
- `.with_transparent(true)` - Enable alpha channel transparency
- `.with_always_on_top(false)` - Allow other windows to cover Clippy
- Window size matches current sprite frame dimensions (auto-resize when frame changes)

**Platform-Specific Transparency:**
- **Windows**: Handled via DWM (Desktop Window Manager), works out of box
- **macOS**: Native NSWindow transparency support
- **Linux X11**: Compositor-based transparency (RGBA window)
- **Linux Wayland**: Compositor transparency, but with positioning limitations (see below)

### Context Menu (Right-Click to Quit)

Implement native context menus per platform:

**Windows:**
- Use `windows-sys` crate for Win32 API calls
- `CreatePopupMenu()` + `TrackPopupMenu()`
- Single "Quit" menu item

**macOS:**
- Use `cocoa` crate for NSMenu
- Create NSPopUpButton or custom NSMenu
- Single "Quit" item

**Linux:**
- Use `gtk-rs` for GTK-based popup menu
- Requires GTK3/GTK4 runtime on system
- Fallback: listen for Escape key if GTK unavailable

### Wayland Limitations

**Challenge:** Pure Wayland does not allow arbitrary client-side window positioning (security/sandboxing). Compositor controls all window placement.

**Impact:** True "wandering" behavior may not work on pure Wayland systems.

**Solutions:**
1. **Primary approach:** Detect Wayland and use `wlr-layer-shell` protocol if available (allows overlay positioning)
2. **Fallback:** On restricted Wayland, accept compositor placement and warn user that wandering is limited
3. **Alternative:** User can run under XWayland (X11 compatibility layer) for full positioning control

This limitation will be documented in the README for Wayland users.

## Asset Management

### Sprite Acquisition

Clippy sprites will be sourced from:
1. Archive.org Windows XP/Office assistant asset dumps
2. Community sprite sheet repositories
3. Individual PNG frames extracted from original Office files

**Required sprites:**
- Idle animation frames (4-8 frames)
- Walk left animation frames (4-8 frames)
- Walk right animation frames (can mirror walk-left if unavailable)

### Asset Loading

**Option A (Recommended):** Embed sprites in binary using `include_bytes!()` macro
- Sprites stored in `assets/sprites/` directory at build time
- Compiled into binary for zero-dependency distribution
- No runtime file I/O

**Option B:** Load from `assets/` directory at runtime
- Allows users to replace sprites without recompiling
- Requires bundling assets folder with binary

Initial implementation will use **Option A** for simplicity.

### Image Processing

Use `image` crate to:
1. Decode PNG files into `RgbaImage` buffers
2. Handle transparency (alpha channel)
3. Optionally pre-process sprites (resize, color correction)

## Rendering

### Renderer: softbuffer

Use `softbuffer` crate for CPU-based pixel rendering:
- Simple API: direct access to window pixel buffer
- Cross-platform (Windows, macOS, Linux/Wayland, Web)
- No GPU/OpenGL dependencies
- Perfect for low-resolution sprite rendering

**Rendering Pipeline:**
1. Get current frame's `RgbaImage` from `SpriteSheet`
2. Lock `softbuffer` surface pixel buffer
3. Copy RGBA pixel data to buffer, handling alpha blending
4. Present buffer to window

**Performance:** At 128x128 sprite size and 60 FPS, softbuffer can easily handle this without GPU acceleration.

### Transparency Handling

Sprites have alpha channel for transparency:
- Fully transparent pixels (alpha = 0) → don't render (window transparency shows through)
- Fully opaque pixels (alpha = 255) → render normally
- Semi-transparent pixels → blend with background (if softbuffer supports, otherwise treat as opaque)

## Dependencies

Core crates:
- `winit` - Cross-platform windowing
- `softbuffer` - CPU rendering to window
- `image` - PNG decoding and image manipulation
- `rand` - Random number generation for behavior

Platform-specific (conditional compilation):
- `windows-sys` - Windows API bindings (context menu)
- `cocoa` - macOS Cocoa bindings (context menu)
- `gtk` - Linux GTK bindings (context menu, optional)

## Build & Distribution

**Target platforms:**
- Windows 10/11 (x86_64)
- macOS 11+ (x86_64 and aarch64)
- Linux (x86_64) with X11 or Wayland compositors

**Build requirements:**
- Rust 1.75+ (edition 2024 requirement from Cargo.toml)
- Platform SDKs (Windows SDK, Xcode, Linux dev tools)
- GTK3-dev (Linux only, for context menu)

**Distribution:**
- Single native binary per platform
- No installer needed
- Optional: Create .app bundle (macOS), .desktop file (Linux)

## Testing Strategy

**Manual testing focus:**
- Visual verification of animations
- Movement behavior observation
- Screen boundary collision testing
- Right-click context menu on each platform
- Multi-monitor scenarios
- Wayland vs X11 behavior differences

**Automated testing (limited):**
- Unit tests for position calculations and boundary clamping
- State machine transition logic tests
- Sprite loading validation

No CI/CD integration required for initial version - manual testing per platform before release.

## Future Enhancements (Out of Scope)

Possible future additions not in initial implementation:
- Vertical and diagonal movement (up, down, NE, NW, SE, SW)
- Multiple animation states (sleeping, waving, celebrating)
- Configuration file for speed/frequency settings
- System tray icon option
- Click interactions (animations on click)
- Multiple characters or skins
- Sound effects
- Multi-monitor awareness (prefer staying on one monitor)

## Open Questions

None - design approved and ready for implementation planning.
