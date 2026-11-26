# Accessibility

Signex aims to be usable by every engineer who designs electronics, including
people who use the keyboard instead of a mouse, need larger or higher-contrast
text, or rely on assistive technology. This page says what works today, what
does not yet, and how to tell us about a barrier.

## Supported environments

Signex is a desktop application for Windows (x86_64 and ARM64), macOS (Apple
silicon and Intel) and Linux (x86_64 and ARM64). The release packages are
listed in the [README](README.md).

## What works today

- **Keyboard.** Commands have keyboard shortcuts, with selectable shortcut
  profiles. `F1` (or **Help ▸ Keyboard Shortcuts**) lists every working
  shortcut, and **Preferences ▸ Keyboard Shortcuts** lets you change them.
  Shortcuts follow the OS conventions (`⌘` on macOS).
- **Interface scale.** **Preferences ▸ Appearance** scales the whole interface
  from 80 % to 200 %, on top of the operating system's display scale.
- **Light and dark themes.** Graphite Light and Graphite Dark are both
  available.
- **Input devices.** Mouse, trackpad (pinch-zoom and two-finger pan) and mouse
  wheel all work, and the wheel can be set to zoom or scroll.

## Known limitations

- **No screen reader support yet.** The user interface toolkit Signex is built
  on does not expose an accessibility tree, so screen readers cannot read
  Signex's controls. We plan to add it when the toolkit supports it.
- **Not every action can be done from the keyboard alone.** Placing, routing
  and drawing on the canvas still need a pointer for some steps.
- **Colour.** Some schematic and board states are shown by colour. Most also
  have a shape, a label or a status-bar message, but not all of them do yet.

## Reporting a barrier

If something in Signex is hard or impossible for you to use, please
[open an issue](https://github.com/alplabai/signex/issues/new/choose). Say what
you were trying to do, what got in the way, and which assistive technology or
setting you use. Accessibility problems are treated as bugs.
