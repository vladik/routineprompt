# RoutinePrompt

Scheduled visual popup reminders for Windows. Reminders that get your attention.

Built for people who need reminders they genuinely cannot ignore — autistic individuals, ADHD adults, caregivers managing daily routines for someone else, and developers who need forced breaks.

## Features

- **Scheduled reminders** — daily, specific days of the week, or one-time
- **Three popup modes** — Gentle (auto-dismisses), Persistent (click to dismiss), Focus (fullscreen)
- **Photo support** — visual image in every popup
- **System tray** — runs silently in the background, no taskbar clutter
- **Auto-launch** — starts with Windows automatically
- **SQLite persistence** — all reminders stored locally, works offline

## Building from Source

Requires [Rust stable](https://rustup.rs/). The first build clones the Zed monorepo for GPUI (~5–10 minutes).

```bash
git clone https://github.com/vladik/routineprompt
cd routineprompt
cargo run
```

## Project Structure

| Crate | Purpose |
|-------|---------|
| `crates/domain` | Core data types: Reminder, Schedule, DisplayMode |
| `crates/storage` | SQLite persistence via rusqlite |
| `crates/scheduler` | Background thread that fires reminders on schedule |
| `crates/ui` | GPUI windows: reminder list, add/edit form, popups |
| `crates/app` | Entry point, system tray, event loop |

## Stack

- **Desktop**: [Rust](https://www.rust-lang.org/) + [GPUI](https://github.com/zed-industries/zed) (GPU-native UI from the Zed editor)
- **Storage**: SQLite via [rusqlite](https://github.com/rusqlite/rusqlite)
- **System tray**: [tray-icon](https://github.com/tauri-apps/tray-icon) + [muda](https://github.com/tauri-apps/muda)
- **Future backend**: Elixir + Phoenix (Phase 4 — multi-device sync)

## Roadmap

See [docs/product-concept.md](docs/product-concept.md) for the full product vision.

- **Phase 2** ✅ — Working MVP: reminders, scheduler, tray, SQLite, auto-launch
- **Phase 3** — Popup behavior settings, AM/PM picker, installer, website
- **Phase 4** — Pro sync: Phoenix backend, web portal, multi-device management
- **Phase 5** — Mobile: iOS and Android

## Background

Built by [Vladik](https://github.com/vladik), creator of [FirstThenBoard](https://firstthenboard.com) — a free visual schedule tool used by thousands of families. RoutinePrompt extends that work to desktop with scheduled, unavoidable visual prompts.

## License

TBD
