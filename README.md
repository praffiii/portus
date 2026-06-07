# Portus

**Port 3000 is taken. Again. By what?**

Portus lives in your menu bar and shows you everything listening on your machine — port, process, CPU, memory — so you can glance and kill in one click without opening a terminal.

<!-- screenshot or GIF here -->

[Download for macOS →](../../releases/latest)

---

## What's in v0.1 beta

- **See** every TCP listener: port, process name, PID, CPU, memory, and where it came from (`docker` / `system` / `from IDE` / `from Terminal` / `orphan?`)
- **Kill** any process directly from the popover
- **Deduped rows** — dual-stack and `SO_REUSEPORT` processes collapse into one stable row
- **Native feel** — vibrancy popover, no Electron

Coming next: Docker container control, managed dev-service runner with live log tailing.

---

## Install

Grab the `.dmg` from the [latest release](../../releases/latest). The build is ad-hoc signed but not notarized — on first launch macOS will block it:

```
System Settings → Privacy & Security → Open Anyway
```

### Build from source

Requirements: Rust stable, Node.js 24, pnpm 10.

```bash
git clone https://github.com/<you>/portus
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
