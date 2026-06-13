# Tihulu Media Source Controller

A Pop!_OS / COSMIC media source controller for choosing exactly which media source your keyboard media keys control.

The idea is simple: choose one active source, then **Previous**, **Play/Pause**, **Next**, **Stop**, **Volume**, and **Mute** always control that selected source.

## One-line install

```bash
curl -fsSL https://raw.githubusercontent.com/Tihulu/tihulu-media-source-controller/main/scripts/install.sh | bash
```

The installer works on Pop!_OS and other apt-based distributions. It installs the required packages, builds the release binary, installs it to `/usr/bin`, and installs the COSMIC applet desktop entry to `/usr/share/applications`.

## Add it to the COSMIC panel

After installing, open:

```text
Settings → Desktop → Panel → Add applet
```

Search for:

```text
Tihulu Media Source Controller
```

If it does not appear immediately, restart the panel or log out and back in.

## Why this exists

On Linux desktops, media keys sometimes control the wrong player when multiple apps are open. For example, Spotify may be playing, but the keyboard Play/Pause key may affect Firefox, VLC, a paused player, or another MPRIS client.

This project is designed to make that behavior explicit and predictable:

- choose one active source
- route media commands to that source
- switch sources from the COSMIC panel
- keep the panel icon compact with Previous / Play-Pause / Next controls
- use notifications when the active target changes

## Current status

This repository contains the first working GUI shell and backend prototype.

The backend uses `playerctl` to target MPRIS players by name. The desktop entry is marked as a COSMIC applet so it appears in the COSMIC panel applet picker.

## Applet behavior

### Panel icon

The applet should show compact media controls directly in the panel:

```text
| Previous | Play/Pause | Next |
```

The active source indicator appears under the panel control group.

### Source picker

Clicking the applet opens a source picker:

- Active Source
- Available Sources
- Apps: Spotify, VLC, Firefox, YouTube Music, etc.
- Manage Sources

### Now Playing panel

The Now Playing view shows:

- active source
- track title and artist
- progress bar
- Previous / Play-Pause / Next controls
- Change Source button

### Settings

Settings stay minimal:

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

Open the GUI:

```bash
tihulu-media-source-controller
```

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

Until direct media-key interception is implemented, bind these commands in desktop keyboard shortcuts:

| Action | Command |
| --- | --- |
| Play / Pause | `tihulu-media-source-controller play-pause` |
| Next Track | `tihulu-media-source-controller next` |
| Previous Track | `tihulu-media-source-controller previous` |
| Stop | `tihulu-media-source-controller stop` |
| Next Source | `tihulu-media-source-controller cycle` |

## Troubleshooting

Check that the applet desktop entry is installed:

```bash
ls /usr/share/applications/com.github.tihulu.TihuluMediaSourceController.desktop
```

Check that it is marked as a COSMIC applet:

```bash
grep -E 'X-CosmicApplet|Categories|Exec' /usr/share/applications/com.github.tihulu.TihuluMediaSourceController.desktop
```

Expected output includes:

```text
Categories=COSMIC
Exec=tihulu-media-source-controller
X-CosmicApplet=true
```

## Development

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
```

## License

GPL-3.0-or-later
