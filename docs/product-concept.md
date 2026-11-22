# RoutinePrompt — Product Concept

## The Problem

People with autism, ADHD, and cognitive differences struggle with routine adherence — not because they don't want to follow routines, but because their brain doesn't self-prompt. Caregivers and parents spend enormous energy giving verbal reminders constantly, every single day. For a parent whose child uses computers and iPads all day, this is an endless daily burden with no end in sight.

The same problem exists at lower intensity for neurotypical people — developers who forget to take breaks, adults who skip meals, anyone who needs a pattern interrupted and cannot rely on internal memory to trigger the right behavior at the right time.

No existing app solves this well on desktop. Phone apps are ignorable. Browser notifications are trained out. What is needed is something that stops the person, shows them exactly what to do, and requires a real response before they can continue.

## What RoutinePrompt Is

A scheduled visual popup system for Windows (and later Mac, iOS, Android) that fires at defined times, interrupts whatever the user is doing, shows a visual reminder with optional photo, and requires a real response before it goes away.

It is not a gentle nudge. It is not a notification badge. It is a visual prompt — the same mechanism used in ABA therapy and special education — delivered digitally, on a schedule, across devices. The person managing the reminders (a caregiver, parent, or the user themselves) defines what appears, when it appears, how long it stays, and what the person must do to dismiss it.

## Popup Behavior — Real Scenarios

Each reminder is not just open/close. It has a full behavior profile defined by the person who created it.

**Scenario 1 — Simple acknowledgement.** Reminder fires, person sees it, clicks OK, done. Works for simple tasks like "brush your teeth."

**Scenario 2 — Person not at the computer.** Reminder fires but nobody is there. The popup stays on screen indefinitely and re-fires every X minutes until acknowledged. The caregiver can see in the analytics log that it was never acknowledged.

**Scenario 3 — Countdown timer.** "Time to eat — you have 15 minutes." Popup shows a live countdown. When the timer hits zero it can re-prompt, escalate display mode, or simply log that time ran out.

**Scenario 4 — Forced minimum time.** Person clicks OK immediately without doing the task. The OK button is locked for X minutes — cannot dismiss until time passes. Used for tasks like eating, resting, or any activity that takes a minimum duration.

**Scenario 5 — Escalation chain.** Person ignores a gentle popup. After X minutes with no response it escalates to persistent. Still no response — escalates to fullscreen. Automatic, no caregiver action needed.

**Scenario 6 — Fullscreen is optional, not default.** Fullscreen mode is a choice per reminder, not the default behavior. A developer setting a break reminder might want persistent but not fullscreen. A caregiver setting a bedtime reminder for a child who ignores everything might need fullscreen.

## Popup Settings Per Reminder

**Appearance**
- Display mode: Gentle (small, corner) / Persistent (stays until dismissed) / Fullscreen (cannot be ignored)
- Show image or photo: yes / no
- Show countdown timer: yes / no, with duration in minutes

**Dismissal rules**
- Can be closed immediately: yes / no
- Minimum time before OK button appears: none / X minutes
- Required action: simple OK / must wait for timer

**If not acknowledged**
- Re-fire after: X minutes
- Escalate display mode after: X minutes of no response
- Maximum re-fires: unlimited / X times

**Recurrence**
- Schedule: daily / specific days of week / one-time
- Keep re-firing until acknowledged: yes / no

## Core Features

**Reminder engine**
- Title and optional photo or image
- Scheduled: daily, specific days of week, or one-time
- Full behavior profile per reminder (see Popup Behavior above)
- Fires reliably even when app is minimized to system tray

**Management UI**
- iPhone alarm-style list: simple, familiar, no learning curve
- Add, edit, delete, toggle on/off per reminder
- Runs in system tray only — no taskbar presence
- Auto-launches on Windows startup
- Left-click tray icon reopens window; right-click to quit

**Who it is for**
- Autistic individuals and others who need visual prompts for daily routine
- Parents and caregivers who currently give these reminders manually every day
- Developers and office workers who want forced breaks they cannot ignore
- Anyone who needs a reminder they genuinely cannot dismiss without noticing

**Multi-device sync (Pro)**
- Reminders sync across all installed devices
- Change on one device, updates everywhere within minutes
- Offline-first: SQLite local database works without internet, syncs when online
- Caregiver manages child's devices from web portal without touching them

**Web portal (Pro)**
- Manage all devices from a browser, no app install needed
- Designed for caregivers, parents, and teachers
- See all devices and all schedules in one place
- Mobile-friendly: works from a phone browser

## Analytics

Analytics are available in both free and Pro tiers — same data, two levels of presentation.

**Free — built into the desktop app**
- Per reminder: fired / acknowledged / ignored / time taken to acknowledge
- Last 7 or 30 days
- Stored in local SQLite, always available offline
- Tells a caregiver which reminders are working and which are being ignored

**Pro — web portal dashboard**
- All devices in one view
- Longer history
- Cross-device patterns: she acknowledged breakfast on all devices but ignored bedtime on the laptop
- Data export

No push notifications to the caregiver's phone — the log answers the same question without added complexity. The caregiver opens the portal, sees what happened, adjusts the reminder settings if needed.

## Business Model

The free tier is genuinely useful standalone. This builds trust, drives organic adoption, and lets caregivers try the app before committing. The Pro tier solves a specific daily pain — managing multiple devices — that caregivers will pay to solve.

| Feature | Free | Pro |
| --- | --- | --- |
| Unlimited reminders | ✓ | ✓ |
| All popup modes and behavior settings | ✓ | ✓ |
| Basic analytics in the app | ✓ | ✓ |
| SQLite local storage, works offline | ✓ | ✓ |
| Multi-device sync | ✗ | ✓ |
| Web portal for remote management | ✗ | ✓ |
| Cross-device analytics dashboard | ✗ | ✓ |
| Caregiver manages child's devices remotely | ✗ | ✓ |

**Target Pro price:** $5/month or $45/year.

**Primary paying audience:** Caregivers and parents managing routines for another person across multiple devices. This is a real daily problem they will pay to solve — it directly replaces hours of manual reminder-giving every week.

**Secondary paying audience:** Developers and productivity users who want sync across work and home machines.

## Technical Architecture

**Desktop client — Rust + GPUI**
- Single binary, no runtime dependencies, no installer complexity
- GPU-rendered popups via GPUI (from Zed monorepo) — appear instantly, never lag behind a slow runtime
- SQLite local database via rusqlite — works fully offline
- Background scheduler thread — fires reminders even when UI is hidden in tray
- System tray only, no taskbar presence, auto-launches on Windows startup
- Background sync thread polls Phoenix API periodically when online

**Sync backend — Elixir + Phoenix**
- REST API for reminder CRUD and device sync
- Offline-first: client works without server, syncs on reconnect
- Conflict resolution: last-write-wins per reminder (simple, correct for this use case)
- PostgreSQL for server-side storage
- Scales to thousands of users on a single small server — Elixir handles concurrency cheaply

**Web portal — Phoenix LiveView**
- No separate JavaScript frontend needed
- Caregiver logs in, sees all devices, edits schedules, reads analytics
- Mobile-friendly: works from a phone browser
- Same Phoenix app as the API — no extra deployment

**Platforms (phased)**
- Windows — now
- Mac — Phase 3 or 4 (GPUI runs on Mac, minor porting work)
- iOS / Android — Phase 5 (Rust core logic + native UI or Tauri)

## Phases

| Phase | What | Status |
| --- | --- | --- |
| 1 | GPUI popup proof of concept — fullscreen window, photo, acknowledge button | Done |
| 2 | Full desktop MVP — reminder list, scheduler, SQLite, tray, auto-launch | Done |
| 3 | Public release — popup behavior settings, AM/PM picker, app icon, installer, website | Next |
| 4 | Pro sync — Phoenix backend, web portal, multi-device, analytics dashboard | Planned |
| 5 | Mobile — iOS and Android | Future |

**Phase 3 priorities before public release:**
- Popup behavior settings: countdown timer, minimum dismissal time, escalation chain, re-fire on no response
- AM/PM time picker (24h is unusable for general audience)
- Real app icon (.ico, not an indigo square)
- Windows installer (.exe, not "run cargo")
- routineprompt.com website with simple landing page and free download
- Thorough popup reliability testing — edge cases, nothing should block a reminder from firing

**Biggest risk across all phases:** The gap between "works on one PC" and "works reliably for 1000 families" is popup reliability. If a reminder fires late, misses, or the app crashes silently, a caregiver loses trust immediately and permanently. Reliability is the product.
