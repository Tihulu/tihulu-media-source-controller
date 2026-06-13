# UI Specification

## Product name

Tihulu Media Source Controller

## Main rule

One selected source controls all media keys.

There is no setting to enable or disable this behavior because it is the core purpose of the applet.

## Panel applet

The panel icon should not be a single generic symbol. It should be a compact media-control group:

```text
Previous | Play/Pause | Next
```

Recommended panel states:

- idle: outline controls
- active: purple border or purple underline
- playing: animated/equalizer indicator near the active source
- unavailable source: muted/disabled style

## Source picker

Header:

```text
Select Active Source
```

Sections:

- Active Source
- Available Sources
- Manage Sources

Each source row includes:

- app/device icon
- source name
- status text
- optional now-playing metadata
- selected state

## Now Playing panel

Header:

```text
Now Playing
```

Main content:

- app icon
- app name
- title / artist
- progress bar
- Previous / Play-Pause / Next
- Change Source button

## Settings

Settings should be short and practical:

- Remember last source
- Show notification on source change
- Open Source Picker shortcut
- Next Source shortcut
- Previous Source shortcut

Avoid settings that make the purpose unclear.
