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
- **He reacts**: click him or press Space and he does one of his 30 gestures.
- **Ask him things**: press Enter for a speech balloon with a prompt box. He
  talks to any OpenAI-compatible server and streams the reply into the
  balloon, thinking and writing while it arrives. See [Chat](#chat).
- **Settings modal**: press F2 (or the balloon's *Settings* button) to set the
  server, model, key, size, and always-on-top while he's running. Changes save
  to a config file. See [Settings](#settings).
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
--base-url <URL>     OpenAI-compatible server, e.g. http://localhost:11434/v1
--model <NAME>       Model to chat with, e.g. llama3.2 or gpt-4o-mini
--api-key <KEY>      Bearer token, if the server needs one
-h, --help           Print the options
-V, --version        Print the version
```

The sprite is 124 by 93 pixels; with the default scale it is 248 by 186
logical pixels, and it follows your display scaling.

### Controls

| Action                          | Effect                                        |
| ------------------------------- | --------------------------------------------- |
| Drag with the left mouse button | Move Clippy                                   |
| Left click or Space             | Clippy does something                         |
| Enter                           | Open the chat balloon and ask him             |
| F2                              | Open the settings modal                       |
| Right click                     | Menu: *Ask Clippy…* / *Settings…* / *Animate!* / *Quit* (Windows, macOS); opens the chat on Linux |
| Escape                          | Close the balloon/settings, or quit           |
| Home                            | Bring Clippy back to the corner               |
| Q                               | Quit                                          |

Closing the window through the system (Alt+F4, Cmd+Q, the window manager's
close action) also quits.

## Chat

Press **Enter** and a speech balloon opens above Clippy with a prompt box.
Type a question, press Enter (or click **Send**), and his reply streams into
the balloon while he thinks and writes. Escape closes the balloon.

Chat works with any server that speaks the OpenAI `/chat/completions` API —
[Ollama](https://ollama.com), LM Studio, llama.cpp's server, vLLM, LiteLLM,
OpenAI itself, and so on. Point Clippy at one with flags or the environment
(flags win):

```bash
# A local server (no key needed)
clippy-app --base-url http://localhost:11434/v1 --model llama3.2

# OpenAI
OPENAI_BASE_URL=https://api.openai.com/v1 \
OPENAI_MODEL=gpt-4o-mini \
OPENAI_API_KEY=sk-... \
clippy-app
```

The environment variables are `OPENAI_BASE_URL`, `OPENAI_MODEL` and
`OPENAI_API_KEY`. Until a base URL and a model are set, chat stays disabled and
the balloon (and the settings modal) say how to turn it on — Clippy still keeps
you company. Requests stream on a background thread, so the animation never
stalls while he waits for an answer, and closing the balloon or asking again
cancels a reply in flight.

Rather than remember flags, use the settings modal below.

## Settings

Press **F2** (or the balloon's *Settings* button, or *Settings…* in the
right-click menu on Windows and macOS) to open a modal where you can set the
chat server, model, and key, the sprite size, and always-on-top while Clippy is
running. **Save** applies the changes immediately — chat turns on without a
restart — and writes them to a config file:

- **Linux**: `~/.config/clippy-app/config.toml` (or `$XDG_CONFIG_HOME`)
- **macOS**: `~/Library/Application Support/clippy-app/config.toml`
- **Windows**: `%APPDATA%\clippy-app\config.toml`

The file is TOML and hand-editable:

```toml
scale = 3
always_on_top = true

[chat]
base_url = "http://localhost:11434/v1"
model = "llama3.2"
# api_key = "sk-..."   # stored in plain text; omitted when empty
```

Settings resolve in this order, highest priority first: **command-line flags →
`OPENAI_*` environment variables → config file → built-in defaults**. So a flag
or env var you pass at launch always wins over the saved file, and the file
wins over the defaults. A missing or malformed file is ignored (Clippy just
uses the defaults and says so on stderr).

## Platform notes

- **Wayland**: Wayland does not let applications choose a window position or
  keep a window above others, so the compositor decides where Clippy appears
  and whether other windows can cover him. Drag him where you want him. To
  get placement and always-on-top back, run him through XWayland:
  `WAYLAND_DISPLAY= ./target/release/clippy-app`.
- **Linux**: there is no native right-click menu (the window has no GTK
  counterpart), so right-clicking opens the chat balloon instead. Quit with
  Escape (when the balloon is closed) or Q.
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
- The chat balloon and the settings modal are `egui`, drawn by `egui_glow`
  into the same GL context as Clippy (they share one `glow` context), so they
  stay part of the one see-through window and follow him when dragged. The
  window grows above him to hold whichever panel is open. `src/chat/` builds
  the request and streams the reply with `ureq` on a worker thread, parsing the
  Server-Sent Events; tokens reach the UI through the winit event loop.
- `src/settings.rs` resolves settings across the command line, environment,
  and the TOML config file, and reads and writes that file (`toml` + `dirs`).

The software renderer used by the first version of this project (`softbuffer`)
was replaced because its Wayland backend has no alpha channel.

## Tests

```bash
cargo test
```

The tests cover the animation data (every frame and branch index it refers
to exists), the player's stepping rules, Clippy's behaviour over a simulated
45-minute session, sprite compositing and premultiplication, the command
line, settings resolution and the config-file format, and the chat subsystem
— the SSE parser, configuration resolution, conversation state, and the
streaming client run end to end against an in-process mock server. The window,
OpenGL, and modal UI are exercised by running the app.

## Credits and licensing

The Clippy artwork is copyright Microsoft Corporation. The sprite sheet and
animation tables were extracted from the original Office Assistant by the
[clippy.js](https://github.com/smore-inc/clippy.js) project, whose code is
MIT licensed. This program's own code is MIT licensed; the artwork is
included for personal use and is not covered by that license.
