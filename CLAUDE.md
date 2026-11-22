# RoutinePrompt — Claude Code Instructions

## What This Is

A Windows-first desktop reminder app built with Rust + GPUI. Scheduled visual popups that demand attention and require acknowledgement. Primary user: autistic individuals who need reliable visual prompts for daily routines. Also useful for developers, ADHD adults, and anyone who needs an unavoidable reminder.

Built by Vladik, creator of firstthenboard.com. See `docs/product-concept.md` for full product vision.

## Workspace Structure

```
crates/
  domain/     — Reminder, Schedule, DisplayMode structs (no dependencies)
  storage/    — SQLite CRUD via rusqlite
  scheduler/  — Background thread, minute-based firing, no Tokio
  ui/         — GPUI windows: main_window, popup, text_input
  app/        — Entry point, tray icon, event loop
docs/
  product-concept.md  — Full product vision and roadmap
```

## Build & Run

```bash
cargo build        # first build clones Zed monorepo (~5-10 min)
cargo run
```

## Key Technical Decisions

- **GPUI** from Zed monorepo at rev `2f808b3c` — use this exact rev, do not update without testing
- **`gpui_platform`** crate is required alongside `gpui` — both from same git rev
- **No Tokio** — scheduler uses `std::thread::spawn` + `std::thread::sleep`, keeps binary small
- **SQLite** via `rusqlite` with `bundled` feature — no external SQLite dependency needed
- **System tray** via `tray-icon 0.14` + `muda 0.13` for right-click context menu
- **`QuitMode::Explicit`** — app stays alive when main window closes (tray mode)
- **Auto-launch** via Windows registry `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
- **Offline-first** — SQLite is source of truth, cloud sync (Phase 4) is background only

## GPUI Gotchas — Read Before Touching UI Code

- Entry point: `gpui_platform::application()` not `Application::new()`
- Click handlers require `.id("some-id")` on the div **before** `.on_click()`
- `is_focused()` takes `&Window`, not `&Context` — signature: `focus_handle.is_focused(window)`
- `Bounds::centered(None, ...)` causes "Invalid window handle" errors on Windows — use fixed coordinates instead
- `overflow_y_scroll()` does not exist — use `overflow_hidden()`
- `WindowHandle::update` closure takes 3 args: `|view, window, cx|` not 2
- `AsyncApp::update` returns `()` not `Result` — do not call `.ok()` on it
- Images: `img(Arc::<std::path::Path>::from(p.as_path()))` — type annotation required
- Do **not** call `tracing_subscriber::fmt::init()` — it surfaces noisy GPUI internal errors that are benign
- GPUI logs "Invalid window handle" and DirectX errors to stderr — these are GPUI internals, not our bugs

## GPUI API Quick Reference

```rust
// App entry
gpui_platform::application().with_quit_mode(QuitMode::Explicit).run(|cx: &mut App| { });

// Open window (use fixed Bounds, not Bounds::centered)
cx.open_window(WindowOptions {
    window_bounds: Some(WindowBounds::Windowed(Bounds {
        origin: gpui::Point { x: px(200.0), y: px(150.0) },
        size: size(px(480.0), px(400.0)),
    })),
    ..Default::default()
}, move |_, cx| cx.new(|_| MyView { })).ok();

// Close window from render context
window.remove_window();

// Spawn foreground task
cx.spawn(async move |cx| {
    cx.background_executor().timer(Duration::from_millis(200)).await;
    cx.update(|cx| { /* use App here */ });
}).detach();

// Close window from async task
handle.update(cx, |_view, window, _cx| { window.remove_window(); }).ok();

// Focus
cx.new(|cx| { let focus_handle = cx.focus_handle(); ... })
focus_handle.is_focused(window)  // in render(), window param must not be _window
focus_handle.focus(window, cx)   // in event handler

// Stateful div (required for on_click)
div().id("my-id").on_click(cx.listener(|this, _event, _window, cx| { }))
```

## Data Model

```rust
Reminder {
    id: Uuid, title: String, message: String,
    image_path: Option<String>, sound_path: Option<String>,
    schedule: Schedule, display_mode: DisplayMode,
    enabled: bool, created_at: DateTime<Utc>, updated_at: DateTime<Utc>,
}

Schedule::Daily { at: NaiveTime }
Schedule::Weekly { days: Vec<Weekday>, at: NaiveTime }
Schedule::Once { at: DateTime<Utc> }

DisplayMode::Gentle       // small popup, auto-dismisses after 5s
DisplayMode::Persistent   // stays until clicked
DisplayMode::Focus        // fullscreen, cannot be ignored
```

## Database

- Path: `%APPDATA%/RoutinePrompt/reminders.db`
- Opened in `app/src/main.rs`, passed to `MainWindow` as `ReminderStore`
- Schedule stored as `sched_type` ("daily"/"weekly"/"once"), `sched_time` ("HH:MM"), `sched_days` ("0,1,2" = Mon,Tue,Wed)
- `sync_shared()` in `MainWindow` keeps `Arc<Mutex<Vec<Reminder>>>` in sync for the scheduler thread

## Scheduler

- Runs in a `std::thread`, checks every 10 seconds
- Tracks `last_fired_minute: Option<(u32, u32)>` to fire once per minute
- Sends fired reminders via `std::sync::mpsc::Sender<Reminder>` to GPUI event loop
- GPUI polls the receiver every 200ms in a `cx.spawn` task

## Tray Icon

- Left-click: reopens main window if no windows are open
- Right-click: "Quit RoutinePrompt" via `muda::MenuItem`
- Menu events polled via `muda::MenuEvent::receiver().try_recv()`
- Tray events polled via `tray_icon::TrayIconEvent::receiver().try_recv()`

## Current Status

| Phase | What | Status |
|-------|------|--------|
| 1 | GPUI popup proof of concept | Done |
| 2 | Full desktop MVP: reminders, tray, scheduler, SQLite, autolaunch | Done |
| 3 | Popup behavior settings, AM/PM picker, app icon, installer, website | Next |
| 4 | Pro sync: Phoenix backend, web portal, multi-device, analytics | Planned |
| 5 | Mobile: iOS and Android | Future |

## Phase 3 Priorities

1. **Popup behavior settings** — countdown timer, minimum dismissal time, escalation chain (Gentle → Persistent → Focus), re-fire if not acknowledged
2. **AM/PM time picker** — current 24h spinner (0–23) is confusing for general audience
3. **Real app icon** — `.ico` file, replace indigo square placeholder
4. **Windows installer** — `.exe` installer so users don't need Rust installed
5. **Landing page** — routineprompt.com with free download

## Conventions

- No `unwrap()` on user-facing paths — use `?` or log and continue
- No comments explaining what code does — only comments explaining non-obvious WHY
- No `tracing_subscriber` initialization in main
- Reminder display in list uses `schedule_display()` in `main_window.rs`
- All GPUI window IDs must be unique strings — use descriptive names like "row-{index}"
