# Portus

**Port 3000 is taken. Again. By what?**

Portus lives in your menu bar and shows you everything listening on your machine — port, process, CPU, memory — so you can glance and kill in one click without opening a terminal.

<!-- screenshot or GIF here -->

[Download for macOS →](../../releases/latest)

---

## What's in v0.2 beta

- **See** every TCP listener: port, process name, PID, CPU, memory, and where it came from (`docker` / `system` / `from IDE` / `from Terminal` / `orphan?`)
- **Kill** any process directly from the popover
- **Projects** — open a folder, save tasks, start/stop your own dev services from the menu bar
- **Live logs** — LogPeek streams managed task output and Docker container logs (ANSI cleaned)
- **Docker** — see containers alongside ports; expand a row to tail logs
- **Ports, project-first** — group listeners under saved projects; unsave when you no longer need them
- **Appearance** — light / dark / system, matched to macOS
- **Deduped rows** — dual-stack and `SO_REUSEPORT` processes collapse into one stable row
- **Native feel** — vibrancy popover, redesigned layout, no Electron

---

## Install

Grab the `.dmg` from the [latest release](../../releases/latest). The build is ad-hoc signed but not notarized — on first launch macOS will block it:

```
System Settings → Privacy & Security → Open Anyway
```

### Build from source

Requirements: Rust stable, Node.js 24, pnpm 10.

```bash
git clone https://github.com/praffiii/portus
cd portus
pnpm install
pnpm tauri build --target aarch64-apple-darwin
```

---

## Contributing

```bash
pnpm install        # install JS deps
pnpm tauri dev      # run the full app with hot-reload
pnpm dev            # Vite frontend only at localhost:1420
```

```bash
pnpm check
cd src-tauri && cargo test
cd src-tauri && cargo clippy --all-targets --all-features -- -D warnings
cd src-tauri && cargo fmt --all
```

**Stack:** Tauri v2 · SvelteKit · Rust · `listeners` + `sysinfo` · `bollard` · `tauri-specta`

---

## License

MIT
