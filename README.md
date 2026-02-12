# Global Hotkey Spotify — Control Spotify with Custom Keyboard Shortcuts

[![Version](https://img.shields.io/badge/version-0.8.0-blue)](https://github.com/Sid-V/global_hotkey_spotify/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Build](https://img.shields.io/github/actions/workflow/status/Sid-V/global_hotkey_spotify/main.yml?branch=master)](https://github.com/Sid-V/global_hotkey_spotify/actions)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey)]()
[![Downloads](https://img.shields.io/github/downloads/Sid-V/global_hotkey_spotify/total)](https://github.com/Sid-V/global_hotkey_spotify/releases)

> Control Spotify playback from any app — no media keys required. Perfect for gamers, compact keyboard users, and multitaskers.

Requires **Spotify Premium**.

![Global Hotkey Spotify Screenshot](https://github.com/user-attachments/assets/7376b75f-b42a-4529-84d6-39297598f10c)

---

## Why Global Hotkey Spotify?

| Problem | Solution |
|---|---|
| You're gaming and need to skip a track | Press your custom hotkey — no alt-tabbing |
| Your 60% keyboard has no media keys | Map any key combo (e.g., `Ctrl+Shift+S`) |
| You want separate volume control | Adjusts Spotify's in-app volume, not system volume |
| Toastify is abandoned | Built from scratch in Rust — actively maintained |

## Features

- **Custom global hotkeys** — Play/Pause, Next, Previous, Volume Up, Volume Down
- **Flexible key combos** — Use 0–2 modifiers (Ctrl, Alt, Shift, Cmd) + any key
- **Click-to-record** — Set hotkeys by pressing them, no manual typing
- **In-app volume control** — Independent from your system volume slider
- **System tray** — Minimize to tray, runs silently in the background
- **Auto-start** — Launch on boot so your hotkeys are always ready
- **Cross-platform** — Windows, macOS (Intel + Apple Silicon), Linux
- **Lightweight** — Built with Tauri + Rust, not Electron (~5 MB)
- **Secure auth** — Spotify OAuth 2.0 with PKCE, tokens stored locally

## Installation

Download the latest release for your OS:

**[⬇ Download Latest Release](https://github.com/Sid-V/global_hotkey_spotify/releases/latest)**

| OS | Format |
|---|---|
| Windows | `.msi` or `.exe` (NSIS installer) |
| macOS (Apple Silicon) | `.dmg` |
| macOS (Intel) | `.dmg` |
| Linux | `.deb` or `.AppImage` |

## Quick Start

1. Download and install
2. Launch the app and log in with your Spotify Premium account
3. Test playback using the on-screen buttons
4. Click on a hotkey field, press your desired key combo, then hit **Save**
5. Enjoy controlling Spotify from anywhere — even in full-screen games

> **Note:** Spotify must have an active playback session (at least one song playing in the Spotify app or browser) for hotkeys to work.

## Supported Keys

**Modifiers (0–2):** `Ctrl` · `Alt` · `Shift` · `Cmd` (macOS)

**Keys:**
All letters (A–Z) · All digits (0–9) · F1–F20 · Home · End · PageUp · PageDown · Delete · Backspace · Escape · Tab · PrintScreen · ScrollLock · Pause · Insert · NumLock · and common symbols (`` ` `` `-` `=` `/` `\` `;` `'` `,` `.` `[` `]`)

## Building from Source

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) (LTS)
- [pnpm](https://pnpm.io/)
- Platform-specific deps:
  - **Linux:** `sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`

### Build

```bash
pnpm install
pnpm tauri build
```

The built installer will be in `src-tauri/target/release/bundle/`.

## Comparison

| Feature | Global Hotkey Spotify | Media Keys | Toastify |
|---|---|---|---|
| Custom key combos | ✅ | ❌ (fixed keys) | ✅ |
| Works without media keys | ✅ | ❌ | ✅ |
| In-app volume control | ✅ | ❌ | ✅ |
| Cross-platform | ✅ Win/Mac/Linux | ✅ | ❌ Windows only |
| Actively maintained | ✅ | N/A | ❌ Abandoned |
| Lightweight (non-Electron) | ✅ ~5 MB | N/A | ❌ ~50 MB |
| Open source | ✅ MIT | N/A | ✅ |

## Tech Stack

- **Backend:** [Rust](https://www.rust-lang.org/) + [Tauri v2](https://tauri.app/)
- **Frontend:** [Vue 3](https://vuejs.org/) + TypeScript + [Vite](https://vitejs.dev/)
- **Spotify API:** [rspotify](https://github.com/ramsayleung/rspotify)
- **Hotkeys:** [global-hotkey](https://github.com/nicbou/global-hotkey)

## Contributing

Contributions are welcome! Please open an issue first to discuss what you'd like to change, or submit a pull request directly for bug fixes.

## License

[MIT](LICENSE) — free to use, modify, and distribute.

## Acknowledgments

Inspired by [Toastify](https://github.com/aleab/toastify) — the original Spotify hotkey app for Windows that is no longer maintained.
