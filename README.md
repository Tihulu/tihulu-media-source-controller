# Tihulu Media Source Controller

A Pop!_OS / COSMIC applet concept for choosing exactly which media source your keyboard media keys control.

The idea is simple: choose one active source, then **Previous**, **Play/Pause**, **Next**, **Stop**, **Volume**, and **Mute** always control that selected source.

## One-line install

```bash
curl -fsSL https://raw.githubusercontent.com/Tihulu/tihulu-media-source-controller/main/scripts/install.sh | bash
```

The installer works on Pop!_OS and other apt-based distributions. It installs the required packages, clones the repository, builds the release binary, and installs it to `/usr/local/bin`.

## Why this exists

On Linux desktops, media keys sometimes control the wrong player when multiple apps are open. For example, Spotify may be playing, but the keyboard Play/Pause key may affect Firefox, VLC, a paused player, or another MPRIS client.

This project is designed to make that behavior explicit and predictable:

- choose one active source
- route media commands to that source
- switch sources from the COSMIC panel
- keep the panel icon compact with Previous / Play-Pause / Next controls
- use notifications when the active target changes

## Current status

This repository currently contains the first backend prototype and the UI design direction.

The backend uses `playerctl` to target MPRIS players by name. The next step is the native COSMIC panel applet UI.

## Planned applet behavior

### Panel icon

The panel applet should show compact media controls directly in the panel:

```text
| Previous | Play/Pause | Next |
```

The active source indicator appears under the panel control group.

### Source picker

Clicking the applet opens a source picker:

- Active Source
- Available Sources
- Apps: Spotify, VLC, Firefox, YouTube Music, etc.
- Devices: Bluetooth headphones and system output
- Manage Sources

### Now Playing panel

The Now Playing view shows:

- active source
- track title and artist
- progress bar
- Previous / Play-Pause / Next controls
- Change Source button

### Settings

Settings should stay minimal:

- Remember last source
- Show notification on source change
- Open Source Picker shortcut
- Next Source shortcut
- Previous Source shortcut

There is no switch for whether media keys are captured. The entire app exists to make media keys control the selected source.

## Installation from source

```bash
sudo apt update
sudo apt install -y git playerctl libnotify-bin cargo

git clone https://github.com/Tihulu/tihulu-media-source-controller.git
cd tihulu-media-source-controller
./scripts/install.sh
```

## Quick local usage

List available media sources:

```bash
tihulu-media-source-controller list
```

Select Spotify:

```bash
tihulu-media-source-controller set spotify
```

Control the selected source:

```bash
tihulu-media-source-controller play-pause
tihulu-media-source-controller next
tihulu-media-source-controller previous
tihulu-media-source-controller stop
```

Cycle to the next source:

```bash
tihulu-media-source-controller cycle
```

## Recommended keyboard shortcuts

Until the native COSMIC applet is implemented, bind these commands in desktop keyboard shortcuts:

| Action | Command |
| --- | --- |
| Play / Pause | `tihulu-media-source-controller play-pause` |
| Next Track | `tihulu-media-source-controller next` |
| Previous Track | `tihulu-media-source-controller previous` |
| Stop | `tihulu-media-source-controller stop` |
| Next Source | `tihulu-media-source-controller cycle` |

## Architecture

The project should evolve in three layers:

1. **Backend router**  
   Stores the active source and forwards media commands to it.

2. **MPRIS source model**  
   Detects player name, playback status, metadata, track position, and icon.

3. **COSMIC panel applet**  
   Shows the compact panel controls, source picker, Now Playing view, and settings.

For the long-term version, the best approach is an MPRIS proxy/router: the desktop sends media key events to the controller, and the controller forwards those events to the selected player.

## Development

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## License

GPL-3.0-or-later
