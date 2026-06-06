# T5 Tray and Popover Design

## Scope

T5 adds the macOS menu-bar shell and the first componentized Svelte popover.
It creates the webview lazily, positions it below the tray icon, and renders
typed fixture data through `PortList`, `DockerList`, and `StatusBadge`.

Snapshot wiring, process actions, Docker actions, Specta type generation, and
project/log controls remain out of scope.

## Window Lifecycle

`tauri.conf.json` declares no startup windows. During application setup, Rust
creates only the tray icon and retains the existing background polling state.

The first primary-button tray click creates a `main` webview window at 380 by
520 pixels. The window is transparent, undecorated, non-resizable, always on
top, hidden from the task switcher, and positioned with
`Position::TrayBottomCenter`. Subsequent clicks toggle the retained window.
Losing focus hides it rather than destroying it.

Opening the popover switches polling to active cadence; hiding it restores idle
cadence. macOS uses accessory activation policy so Portus does not show a Dock
icon.

## Frontend Structure

The page is a fixed-width popover with a pinned glance header and one scrollable
body. It uses:

- `StatusBadge.svelte` for shape-plus-color status presentation.
- `PortList.svelte` for two-line listening-process rows.
- `DockerList.svelte` for running and stopped container rows.
- A small typed model and local fixture dataset for T5-only rendering.

The implementation follows `DESIGN.md`: Geist-compatible local system
fallbacks, neutral surfaces, teal interaction accent, semantic status colors,
compact spacing, sticky section headings, and first-class dark mode. The
reference mockup informs hierarchy and density, but its gradients, glowing
dots, terminal prompt, and unimplemented controls are not copied.

## Verification

Rust tests exercise lifecycle decisions independently from Tauri platform
objects: first click creates, later clicks show or hide, and focus loss hides.
Configuration inspection confirms there is no startup window.

Frontend verification covers Svelte/TypeScript diagnostics, production build,
and desktop/mobile-width screenshots for clipping and hierarchy. Manual Tauri
verification confirms the first click creates the webview and later clicks
reuse it. Before-first-open Activity Monitor measurement must remain below the
ADR 0002 idle RAM target of 80 MB with approximately 0% idle CPU.
