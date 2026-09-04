# Real Clippy on the Desktop - Design

**Date:** 2026-09-04
**Status:** Implemented
**Supersedes:** `2026-04-13-clippy-desktop-companion-design.md`

## Overview

Put the real Microsoft Clippy on the screen, with a transparent background, on
every desktop environment: Windows, macOS, Linux X11 and Linux Wayland. The
window shows the original Office Assistant sprites and plays the original
animations.

## Goals

- A real Clippy image with a transparent background, not a placeholder.
- Correct per-pixel transparency on all four windowing systems.
- The original animations, sequenced the way the original player did it.
- Movable by dragging; quits from the keyboard and, where native menus exist,
  from a right-click menu.
- Small, dependency-light, embedded assets.

## Non-goals

- Wandering across the screen. The original sprites have no walk cycle, and
  Wayland does not allow client-side positioning at all; sliding sprites
  looked broken. Dragging replaces it.
- Sound, speech balloons, configuration files, a tray icon.

## Key decision: OpenGL instead of a software buffer

The first version drew with `softbuffer`. Its documented pixel format is
`0RGB` with the top byte required to be zero, and its Wayland backend
allocates `wl_shm` buffers in `Xrgb8888`, which has no alpha channel. A
transparent window was therefore impossible on Wayland and undefined
elsewhere.

`glutin` provides an OpenGL context whose config has an 8-bit alpha channel on
all four platforms:

| Platform | Mechanism                                                                                     |
| -------- | --------------------------------------------------------------------------------------------- |
| Windows  | WGL config with alpha; winit enables DWM blur-behind with an empty region so alpha is honoured |
| macOS    | CGL config with `transparency`; winit makes the NSWindow non-opaque with a clear background   |
| X11      | GLX config whose visual is 32-bit; `glutin-winit` creates the window with that visual          |
| Wayland  | EGL config with alpha; the compositor composites the surface with premultiplied alpha          |

Rendering: clear to transparent black, draw one textured quad with
`ONE, ONE_MINUS_SRC_ALPHA` blending and a premultiplied-alpha texture,
nearest-neighbour sampling. Shaders are selected at run time for GLSL ES 1.00,
GLSL 1.10 (OpenGL 2.1 to 3.1) or GLSL 1.50 (OpenGL 3.2+, including macOS
core profiles). Context creation tries the newest OpenGL, then OpenGL ES,
then OpenGL 2.1.

## Assets

- `assets/clippy/map.png`: the clippy.js sprite sheet (3348 x 3162, 124 x 93
  cells, transparent background). Re-encoded losslessly with zopflipng;
  decodes to the same RGBA pixels.
- `assets/clippy/agent.json`: the clippy.js `agent.js` tables converted to
  plain JSON, with the sound references removed.
- Both are embedded with `include_bytes!` / `include_str!` and validated at
  startup (every cell inside the sheet, every frame index in range).

## Architecture

```
main.rs      options -> app::run
app.rs       winit ApplicationHandler: window, glutin surface, input, timing
renderer.rs  glow pipeline: shaders, quad, frame texture, draw
sprite.rs    sheet decoding, per-frame compositing to premultiplied RGBA
agent.rs     data model + validation
animator.rs  one animation's stepping rules (port of the original player)
companion.rs behaviour: greeting, rest, idle fidgets, reactions, queue
platform.rs  native context menu (muda) on Windows/macOS; stub on Linux
```

### Animator (port of the original stepping rules)

- Each frame is shown for its own duration; a step chooses the next frame.
- Leaving a frame may take a weighted random branch (`branching`).
- After `exit()`, frames follow their `exitBranch` instead, which leads to the
  final frame.
- `useExitBranching` animations hold the frame before last until told to
  exit.
- The animation reports `Exited` the first time the cursor reaches the last
  frame.
- Zero-duration frames are stepped through within one `advance`, bounded to
  256 steps.
- A late `advance` moves one frame and reschedules from now, so a stalled
  machine does not burst through frames.

### Companion (behaviour)

- Start: `Greeting`.
- Then: rest 2-6 s on the last frame, play a random `Idle*` animation (never
  the same twice in a row), rest again, and so on.
- Poke (click, Space, Enter, menu): while resting, play a random gesture at
  once; during an idle animation, exit it gracefully (bounded by 1.5 s) and
  then play the gesture; during a gesture, queue at most one more.
- Never picked at random: `Hide`, `Show`, `GoodBye`, `RestPose`.

### Event loop

`ControlFlow::WaitUntil(next deadline)`: the process sleeps until the next
frame or the end of a rest. Redraws happen only when the frame changes, when
the window is resized, or when the system asks.

### Input

- Left press + 4 px of motion: `Window::drag_window()` (compositor-driven
  move on Wayland, `_NET_WM_MOVERESIZE` on X11, `WM_NCLBUTTONDOWN` on
  Windows, `performWindowDragWithEvent` on macOS). The press is consumed
  because the OS swallows the release during a drag.
- Left release without motion: poke.
- Right release: native menu (Windows, macOS) or poke (Linux).
- Escape / Q: quit. `CloseRequested` (Alt+F4, Cmd+Q, WM close): quit.

### Placement

Bottom-right of the primary monitor with a 48 x 96 logical-pixel margin,
computed in physical pixels from the monitor's scale factor. Wayland reports
no primary monitor and ignores positions; the compositor places the window.

## Verification

- `cargo test`: data validation, animator rules (synthetic agent), behaviour
  over a simulated 45-minute session with pokes, compositing, options.
- `cargo check --target` for `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`
  and `x86_64-apple-darwin` (type-checks the platform-specific code paths).
- Manual on Linux: run under X11 (XWayland) and Wayland (labwc) over a solid
  backdrop; a script matches the on-screen pixels against the sprite sheet
  and confirms the rest of the window shows the backdrop.
