# Clippy Desktop Companion

A cross-platform desktop companion application featuring Microsoft Clippy that wanders around the screen with random walk behavior and idle animations.

## Features

- **Random wandering behavior**: Clippy walks around your screen in random directions
- **Smooth animations**: Cycles through idle and walking sprite frames
- **Cross-platform**: Works on Windows, macOS, and Linux (X11/Wayland)
- **Lightweight**: Native CPU rendering with minimal resource usage
- **No dependencies**: Self-contained binary with embedded sprites

## Building

### Requirements

- Rust 1.75 or later
- Platform-specific dependencies:
  - **Windows**: Windows SDK (usually installed with Visual Studio)
  - **macOS**: Xcode command line tools
  - **Linux**: X11 or Wayland development libraries

### Linux: GTK Menu (Optional)

By default, Linux uses Escape key to quit. For native GTK context menu:

```bash
# Install GTK development libraries (Ubuntu/Debian)
sudo apt-get install libgtk-3-dev

# Build with gtk-menu feature
cargo build --release --features gtk-menu
```

### Standard Build

```bash
cargo build --release
```

Binary will be in `target/release/clippy-app` (or `clippy-app.exe` on Windows).

## Running

Simply run the executable:

```bash
./target/release/clippy-app
```

Clippy will appear on your screen and start wandering around!

## Controls

- **Right-click**: Show context menu with Quit option (Windows/macOS/Linux with gtk-menu)
- **Escape key**: Quit (all platforms, primary method on Linux without gtk-menu)

## Platform Notes

### Linux - Wayland

Pure Wayland compositors control window positioning for security. This limits the wandering behavior. You'll see this warning on startup:

```
Warning: Running on Wayland without layer-shell support.
Window positioning is controlled by compositor - wandering behavior limited.
Run under XWayland for full positioning control.
```

**Solution**: Run under XWayland for full positioning:
```bash
WAYLAND_DISPLAY= ./clippy-app
```

Or use X11 session instead of Wayland.

### Replacing Sprites

Sprite images are embedded in the binary at compile time from `assets/sprites/`:

- `idle.png` - Idle animation (currently 1 frame)
- `walk_left_0.png` through `walk_left_3.png` - Walking animation (4 frames)

To use different sprites:

1. Replace PNG files in `assets/sprites/`
2. Keep dimensions at 128x128 pixels
3. Use PNG format with alpha channel for transparency
4. Rebuild the application

The current placeholders are simple colored rectangles. Replace them with actual Clippy sprites or any other character!

## How It Works

**Architecture**:
- `winit` for cross-platform windowing
- `softbuffer` for CPU-based pixel rendering
- `image` crate for PNG sprite loading
- Platform-specific APIs for context menus

**Behavior**:
- Every 2-5 seconds: 60% chance to start walking, 40% stay idle
- Walking duration: 1-3 seconds
- Movement speed: 3 pixels per frame at 60 FPS
- Animation: 10 FPS frame cycling (100ms per frame)

## License

This is a demonstration project. Replace the placeholder sprites with your own or properly licensed sprites before distribution.

## Future Enhancements

Possible improvements (not currently implemented):

- Vertical and diagonal movement
- More animation states (sleeping, waving, celebrating)
- Configuration file for speed/frequency
- Multiple characters or skins
- Sound effects
- Better Wayland support via layer-shell protocol
