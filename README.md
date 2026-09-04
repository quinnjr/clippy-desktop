# Clippy

Microsoft Clippy, the Office Assistant, living on your desktop.

The app shows the original Clippy sprites and plays the original Office
Assistant animations in a borderless, transparent, always-on-top window. It
runs on Windows, macOS and Linux (X11 and Wayland).

## Features

- **The real Clippy**: the sprite sheet and frame tables extracted from the
  original Office Assistant (`CLIPPIT.ACS`), with a fully transparent
  background. All 43 original animations are included.
- **He fidgets**: greets you on startup, then plays his idle animations at
  random intervals, exactly the way the original player sequenced them
  (frame durations, weighted random branching, exit branches).
- **He reacts**: click him, press Space or Enter, or pick *Animate!* from the
  right-click menu and he does one of his 30 gestures.
- **Drag him anywhere** with the left mouse button.
- **Real transparency everywhere**: rendering goes through OpenGL with an
  alpha-capable framebuffer, so only Clippy's outline is visible on
  Windows (DWM), macOS, X11 (with a compositor) and Wayland.
- **Light**: the window only wakes up when the next frame is due.
- **Self-contained**: sprites and animation data are embedded in the binary.

## Building

Requirements: Rust 1.88 or newer.

- **Windows**: the MSVC toolchain (Visual Studio Build Tools).
- **macOS**: Xcode command line tools.
- **Linux**: no development packages are needed; X11, Wayland and OpenGL
  libraries are loaded at run time and come with every desktop
  (Mesa or vendor drivers).

```bash
cargo build --release
```

The binary is `target/release/clippy-app` (`clippy-app.exe` on Windows).

## Running

```bash
./target/release/clippy-app
```

Clippy appears in the bottom-right corner of your primary screen (where the
system allows applications to place windows; see the Wayland note below).

### Options

```
--scale <N>          Size multiplier for the sprite, 1 to 8 (default: 2)
--no-always-on-top   Let other windows cover Clippy
-h, --help           Print the options
-V, --version        Print the version
```

The sprite is 124 by 93 pixels; with the default scale it is 248 by 186
logical pixels, and it follows your display scaling.

### Controls

| Action                          | Effect                              |
| ------------------------------- | ----------------------------------- |
| Drag with the left mouse button | Move Clippy                         |
| Left click, Space or Enter      | Clippy does something               |
| Right click                     | Menu: *Animate!* / *Quit* (Windows, macOS) |
| Escape or Q                     | Quit                                |

Closing the window through the system (Alt+F4, Cmd+Q, the window manager's
close action) also quits.

## Platform notes

- **Wayland**: Wayland does not let applications choose a window position or
  keep a window above others, so the compositor decides where Clippy appears
  and whether other windows can cover him. Drag him where you want him. To
  get placement and always-on-top back, run him through XWayland:
  `WAYLAND_DISPLAY= ./target/release/clippy-app`.
- **Linux**: there is no native right-click menu (the window has no GTK
  counterpart), so right-clicking makes Clippy do something instead. Quit
  with Escape or Q.
- **X11**: transparency needs a compositing window manager, which every
  current desktop environment provides.
- **Windows**: release builds are GUI programs, so nothing is printed to a
  console; the options are listed above.

## How it works

- `winit` creates the window; `glutin-winit` creates it with an OpenGL
  config that has an alpha channel (on X11 that means a 32-bit visual).
- `glow` draws one textured quad over a transparent clear colour with
  premultiplied-alpha blending, using nearest-neighbour sampling so the pixel
  art stays crisp at any scale.
- `assets/clippy/agent.json` holds the animation tables; `src/animator.rs`
  is a port of the original player's stepping rules, and
  `src/companion.rs` decides what Clippy does next (greet, rest, fidget,
  react to a poke).
- `assets/clippy/map.png` is the sprite sheet: 902 cells of 124 by 93
  pixels with a transparent background.
- Native menus on Windows and macOS come from `muda`.

The software renderer used by the first version of this project (`softbuffer`)
was replaced because its Wayland backend has no alpha channel.

## Tests

```bash
cargo test
```

The tests cover the animation data (every frame and branch index it refers
to exists), the player's stepping rules, Clippy's behaviour over a simulated
45-minute session, sprite compositing and premultiplication, and the
command line. The window and OpenGL code is exercised by running the app.

## Credits and licensing

The Clippy artwork is copyright Microsoft Corporation. The sprite sheet and
animation tables were extracted from the original Office Assistant by the
[clippy.js](https://github.com/smore-inc/clippy.js) project, whose code is
MIT licensed. This program's own code is MIT licensed; the artwork is
included for personal use and is not covered by that license.
