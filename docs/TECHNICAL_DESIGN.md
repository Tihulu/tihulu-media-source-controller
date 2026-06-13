# Technical Design

## Goal

Make keyboard media keys deterministic on Pop!_OS / COSMIC.

The user selects one active source. Every media key command targets that source until the user changes it.

## Source types

The applet should support three categories:

1. **MPRIS players**
   - Spotify
   - VLC
   - Firefox
   - Chromium-based browsers
   - YouTube Music
   - COSMIC Media Player

2. **Audio output context**
   - system output
   - Bluetooth headphones
   - USB DACs

3. **Virtual active target**
   - a remembered source
   - an automatically selected currently-playing source
   - a pinned source

## Backend prototype

The first backend uses `playerctl`:

```bash
playerctl --list-all
playerctl --player spotify play-pause
playerctl --player spotify next
playerctl --player spotify previous
```

This gives us a working router before the native applet is finished.

## Long-term implementation

The long-term implementation should avoid relying only on global desktop shortcuts.

A stronger design is an **MPRIS proxy/router**:

1. The controller exposes its own MPRIS player interface.
2. The desktop sees the controller as the media key target.
3. The controller forwards Play/Pause, Next, Previous, and Stop to the selected real source.
4. The applet UI updates from the selected source metadata.

This makes the applet behave like a real media target instead of just a shortcut launcher.

## COSMIC panel UI

The panel UI should use the visual direction from `assets/concept.png`:

- dark translucent panel
- compact Previous / Play-Pause / Next control group
- purple active state
- rounded source cards
- one active source at a time
- no unnecessary toggles

## Configuration

Default config path:

```text
~/.config/cosmic-media-source-controller/config.toml
```

Example:

```toml
active_source = "spotify"
remember_last_source = true
show_notifications = true
```

## Open implementation tasks

- Build native COSMIC panel applet shell.
- Replace plain `playerctl` calls with direct D-Bus/MPRIS control.
- Add live player metadata.
- Add Bluetooth connection detection.
- Add source pinning.
- Add proper packaging for Pop!_OS.
- Add CI release artifacts.
