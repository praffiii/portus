# Portus — v1 Implementation Plan

> Output of `/plan-eng-review` on 2026-06-06. Scope: **staged — ship the lean
> "See + Kill" wedge as a real public release first, then build to full v1**
> (ADR 0007, final form; confirmed by cross-model consensus). A throwaway
> de-risking spike (PTY + binary size) runs in week one. Source of truth for
> *what*: `.docs/req/portus-design.md` + `CONTEXT.md` + `docs/adr/0001`–`0013`.

## Locked technical decisions (this review)

| # | Decision | Choice |
|---|----------|--------|
| F1 | Detection stack | `listeners` (port→PID) + `sysinfo` (info + parent map), behind a per-OS trait |
| F2 | Frontend framework | **Svelte** (tiny bundle/RAM, fits ADR 0002) |
| F3 | IPC model | Rust owns adaptive poll cadence + **pushes** snapshot events; frontend signals `set_active`/`set_idle`; actions via `invoke`; logs via per-process `emit` streams |
| F4 | Log rendering | Lightweight ANSI→HTML, not xterm.js (ADR 0008) |
| F5 | Type contract | `tauri-specta` generates TS types from Rust (one source of truth) |
| F6 | Test strategy | Real Rust integration tests + Svelte component tests; **no** full GUI E2E |
| F7 | Poll cost | Resolve info only for listening PIDs per poll; build full process tree **lazily at kill time** |

Plus the grilling ADRs: adaptive polling (0001), resource budget + lazy webview
(0002), login-shell spawn + `.env` (0003, amended for command safety),
Save-as-project for logs (0004), tree-kill + signal escalation (0005), free
Homebrew distribution (0006), staged release (0007), LogPeek ANSI→HTML + escaping
+ backpressure (0008).

**Hardening from the Codex outside-voice review (folded in):**

| ADR | Hardening |
|-----|-----------|
| 0009 | Revalidate process identity (start-time + exe) before any kill/stop — no PID-reuse footgun |
| 0010 | Reconcile Docker host-proxy ports → one container row; never raw-kill the proxy |
| 0011 | Port model + stable row identity (TCP v1; v4/v6 dedup; SO_REUSEPORT; no flicker) |
| 0012 | Privilege strategy: best-effort, honest "needs elevated privileges" state, no root helper |
| 0013 | Managed-process lifecycle state machine + kill-on-quit, no post-crash adoption |

## Library stack (assemble, don't invent)

- **Tauri v2** — shell, tray icon, popover window. `tauri-plugin-positioner`
  (`TrayBottomCenter`) for under-tray placement; NSVisualEffectView vibrancy for
  native popover feel. Reference: `ahkohd/tauri-macos-menubar-app-example`
  (`v2-popover` branch).
- **`listeners`** — cross-platform listening port → PID/name.
- **`sysinfo`** — per-PID CPU/mem/command/cwd + parent PID map.
- **`bollard`** — Docker Engine API over the socket (list + `logs -f`; container
  start/stop is a small later add if wanted).
- **`portable-pty`** — managed-process PTY (Layer 3).
- **`tauri-specta`** — generate typed TS bindings for commands + events.
- **Frontend:** Svelte; a small ANSI→HTML lib (e.g. fancy-ansi) for LogPeek.

## Architecture

```
Frontend (Svelte popover)                     Rust core (tokio)
  on open  ──invoke(set_active)──►  poller: ~2s cadence (ADR 0001)
  on close ──invoke(set_idle)────►  poller: ~5-10s cadence
  render   ◄──emit("snapshot")────  poll loop: {ports, procs(listening only), containers}
  Kill     ──invoke(kill_tree)───►  build descendant set NOW → SIGTERM→grace→SIGKILL → verify port freed
  Start    ──invoke(start_task)──►  $SHELL -l -c, cwd, .env, spawn in own process group (PGRP)
  LogPeek  ◄──emit("log:<id>")────  PTY reader thread (Managed) / `docker logs -f` (container)
  input    ──invoke(send_input)──►  write to PTY stdin

Rust modules:   ports · process · docker · logs · projects · state::poller
  SEE path:     ports, docker(read), process.info, process.kill_tree
  CONTROL path: process.spawn/PTY, logs, projects   (clean boundary, ADR note)
Per-OS:         ports + process behind traits with #[cfg(target_os)] impls
Persistence:    projects.json (serde) in app config dir   (JSON, not SQLite)
Resource gates: binary <15MB · idle RAM <80MB · idle CPU ~0% · lazy webview
```

## Build layers (each compiles, runs, and is fully tested before the next)

**Layer 0 — De-risking spike (week one, throwaway code)**
- Bare Tauri v2 app: spawn one PTY via `$SHELL -l -c`, stream a line to a window,
  measure the **built binary size** (release, target arch). Retires the two
  biggest unknowns (PTY lifecycle + the `<15MB` budget) before committing.
- Also confirm argv + cwd recovery works for the user's own PIDs (for
  Save-as-project, ADR 0004). Output: go/no-go + adjusted budget if needed.

**Layer 0 RESULTS (2026-06-06) — GO.** Ran on `spike/layer-0` (Tauri v2 + Svelte-TS,
all heavy deps linked: bollard + tokio + portable-pty + listeners + sysinfo).
- **PTY pipeline: PASS.** `$SHELL -l -c` spawned in a real PTY, output streamed
  line-by-line to the webview via events. The login shell picked up the Homebrew
  PATH (`node: /opt/homebrew/bin/node`) — direct confirmation of ADR 0003 (env
  matches Terminal, no "command not found").
- **Binary size: PASS with margin.** Release binary 8.4MB; **stripped 6.5MB**;
  whole `.app` 8.5MB — well under the 15MB budget *with bollard linked*.
  Caveat: single-arch (aarch64). A **universal** binary ~doubles (~13–17MB, near
  the line) → prefer per-arch downloads, or accept universal is near-budget.
- **Not yet verified:** argv/cwd recovery for foreign PIDs (KERN_PROCARGS2) — defer
  to the start of Layer 2 when Save-as-project is built.
- DMG bundling failed in this headless run (`bundle_dmg.sh`) — irrelevant; we ship
  via Homebrew cask, not DMG (ADR 0006).

**Layer 1 — See + Kill** (→ **real public release**, post to Sonar #15)
- `ports` (listeners behind trait), `process` (sysinfo info + tree-kill +
  escalation + verify), `docker` (bollard read-only list), `state::poller`
  (adaptive cadence), tray + popover window, `PortList`/`DockerList`/`StatusBadge`.
- Stateless. No persistence, no PTY.

**Layer 2 — Projects + persistence** (locked by `/plan-eng-review` 2026-06-08,
incl. Codex outside-voice; see "Layer 2 locked decisions" below)
- `projects` module, folder picker, `projects.json` round-trip, read
  `package.json` scripts + `docker-compose.yml` services into Tasks,
  spawn-in-process-group **with piped stdout/stderr → read-only ring buffer**
  (no PTY yet), lifecycle state machine, kill-on-quit, Save-as-project from a
  detected process via parent-chain command selection. Processes started here
  are **Portus-started**, not yet **Managed** (no PTY/send-input until Layer 3 —
  see CONTEXT.md tiers).

**Layer 3 — PTY + logs (hardest)**
- `logs` module, `portable-pty`, `$SHELL -l -c` login-shell spawn, `.env`
  auto-load, inline `LogPeek` (ANSI→HTML), send-input, `waiting?` heuristic,
  `docker logs -f` streaming.

**Layer 4 — Quick-run + polish**
- Quick-run box + project selector, empty state (Open folder + Quick-run), error
  states, resource-budget pass.

## Layer 2 locked decisions (2026-06-08, `/plan-eng-review` + Codex outside-voice)

Scope accepted: **full Layer 2** (persistence + discovery + spawn + lifecycle +
kill-on-quit + Save-as-project). Eleven decisions, five of them reversals/refinements
from the Codex outside-voice (all accepted by the user):

| # | Decision | Choice |
|---|----------|--------|
| L2-1 | State ownership | Backend `Arc<Mutex<ProjectRegistry>>` via `app.manage()`; lock held only to copy, released before any `.await`. |
| L2-2 | Transport (← Codex #7) | Project **definitions via commands** (`load_projects`/`save_project`/`add_task`/…). Snapshot carries a **runtime-only** section `managed: Vec<ManagedStatus { project_id, task_id, pid, pgid, lifecycle }>` — never full config. Frontend joins definition + status. |
| L2-3 | Ownership primitive (← Codex #5/#6) | Store **pgid** at spawn. Hot path per tick = `Child::try_wait()` + "is any port-row owner PID a member of our pgid?". **No `all_snapshots()` in the poll hot path** (respects F7 lazy-tree). `descendants_of` stays for manual detected kills only. |
| L2-4 | Observability (← Codex #8) | Spawn with **piped stdout/stderr → per-process ring buffer** (~200 lines, capped). Read-only startup diagnostics. **No PTY, no ANSI parsing, no send-input** (Layer 3). Reverses the original "start blind". |
| L2-5 | Lifecycle | States: `starting` → `running` (port bound) / `running_no_port` / `exited` (code 0) / `crashed` (code ≠ 0). Signals: `try_wait` + pgid-port match + ring-buffer non-empty. |
| L2-6 | `running_no_port` | **Neutral, honesty-marked** state (like `waiting?`), never an error color. Trigger: alive + grace (~10s) + no port on pgid. |
| L2-7 | Kill-on-quit | Spawn `process_group(0)`; on Tauri **`RunEvent::Exit`/`ExitRequested`** (currently plain `.run()` — must be wired) `killpg(pgid, SIGTERM)` → bounded grace → `killpg(SIGKILL)`. No post-crash adoption (Portus SIGKILL → orphans degrade to **Detected** next launch). Test normal quit vs window-hide vs dev shutdown. |
| L2-8 | YAML (← Codex #12) | **`yaml-rust2`** (serde_yaml archived); add to Cargo. Parse `docker-compose.yml` **and** `compose.yml`/`compose.yaml`; extract service names for Task suggestions. |
| L2-9 | Persistence (← Codex #11) | Atomic write (temp + `fs::rename`); `version` field; corrupt file → rename `projects.json.corrupt-<ts>`, start empty, surface a one-line notice. Plus: **canonicalize** folder paths, dedupe project identity by canonical path, tolerate deleted folders, surface write-failure UX. |
| L2-10 | Save-as-project (← Codex #2) | **Walk the parent chain** from the listener PID (via `all_snapshots`, at save-time only) → collect command candidates → user **picks/edits** (default: top non-shell candidate, e.g. `pnpm dev`). cwd from the chosen candidate. Editable string lets the user strip secrets before persisting. |
| L2-11 | Terminology (← Codex #3) | Three tiers **Detected → Portus-started → Managed** (CONTEXT.md updated). Layer 2 code/UI says "Portus-started", never "Managed". |
| L2-12 | Folder picker (← Codex #10) | Add **`tauri-plugin-dialog`** + capability entry (current capabilities allow core + opener only). |

**Module layout (avoid the SEE/CONTROL merge conflict from the parallelization
plan):** new `projects/` module — `store.rs` (persistence), `parse.rs`
(package.json + compose), `spawn.rs` (login-shell spawn + pgid + ring buffer),
`lifecycle.rs` (state machine), `registry.rs` (`ProjectRegistry` + `ManagedStatus`).
Keep `process/` `info`/`kill` separate from any spawn code.

## Failure modes (per new codepath: test? error-handled? user-visible?)

| Codepath | Realistic failure | Test | Error handling | User sees |
|----------|-------------------|------|----------------|-----------|
| ports.scan | EPERM / partial scan | yes | degrade, show partial | partial list, no crash |
| process.kill_tree | EPERM on protected proc | yes | catch, message | "can't kill (permission)" |
| process.kill_tree | port still held after kill | yes | verify + report | "still listening" warning |
| docker.list | socket absent | yes | cache "not detected" | "Docker not detected" |
| projects.json | corrupt/missing file | yes | recover to empty | starts clean, no crash |
| pty.spawn | command not found / bad env | yes | mark crashed | error in LogPeek |
| pty.send_input | process already exited | yes | no-op + status | input ignored, status updates |
| poller | a probe panics | yes | isolate, keep others | stale section, no app crash |
| kill/stop | PID reused since snapshot | yes | revalidate identity, abort | "no longer there", list refreshes (ADR 0009) |
| docker | host-proxy dup row | yes | reconcile to container row | one row; proxy not raw-killable (ADR 0010) |
| kill | protected/other-user proc | yes | gate action, show state | "needs elevated privileges" (ADR 0012) |
| logs | malicious ANSI/OSC in output | yes | escape-then-ANSI, strip OSC | inert text, no injection (ADR 0008) |
| logs | noisy build floods events | yes | ring buffer + throttle | trimmed marker, no RAM blowup (ADR 0008) |
| pty | Portus quits/crashes | yes | kill-on-quit; no adoption | clean exit; ex-managed → Detected (ADR 0013) |
| service | started but never bound port | yes | running_no_port state | neutral honesty-marked state, not false success (ADR 0013, L2-6) |
| spawn | command not found / fails at startup | yes | piped output → ring buffer, mark crashed | crashed status + recent output shows why (L2-4) |
| ring buffer | noisy build floods stdout | yes | capped ring buffer (~200 lines) | trimmed, no RAM blowup |
| save-as | listener PID is child (node), not `pnpm dev` | yes | parent-chain candidates, user picks/edits | reproducible saved Task (L2-10) |
| projects.json | write fails mid-save (disk full) | yes | temp+rename keeps original intact | original projects survive, error surfaced (L2-9) |
| kill-on-quit | window hidden, not quit | yes | only RunEvent::Exit kills, not hide | hiding popover does not kill services (L2-7) |
| set_idle/poll | Mutex held across Docker await | yes | copy under lock, release before await | no poll stall/deadlock (L2-1) |
| poller | overlapping slow Docker call | yes | single-flight, drop late | no reordered/stale snapshots |
| set_idle | missed close event | yes | focus/blur + safety timeout | can't get stuck fast-polling |

**Critical-gap rule:** none may be both silent AND unhandled. The poller must
isolate a panicking probe so one bad source never blanks the whole popover.

## Worktree parallelization

Within a layer, lanes are mostly sequential (shared `state`/types). Across the
**SEE vs CONTROL** boundary there is real parallelism after Layer 1:

| Lane | Modules | Depends on |
|------|---------|------------|
| A (SEE) | ports, docker(read), process.info/kill | — |
| B (UI shell) | tray, popover, Svelte scaffold, specta bindings | A's types |
| C (CONTROL) | projects, logs, pty | Layer 1 done |

Execution: Lane A + B in parallel (B consumes A's generated types). Merge. Then
C. Conflict flag: A and C both touch `process` (info vs spawn) — keep `info`/`kill`
and `spawn`/`pty` in separate files within the module to avoid merge conflicts.

## Implementation Tasks
Synthesized from this review. P1 blocks ship; P2 same-branch; P3 follow-up.

- [x] **T1 (P1, CC: ~30min)** — ports — Implement `listeners`-backed port scan behind a per-OS `PortProbe` trait with a fake impl.
  - Surfaced by: Finding 1. Files: `src-tauri/src/ports/`. Verify: integration test, dummy listener detected.
- [x] **T2 (P1, CC: ~45min)** — process — `sysinfo` info for listening PIDs + lazy descendant-map builder; tree-kill with SIGTERM→grace→SIGKILL + post-kill port verify.
  - Surfaced by: Findings 1, 7; ADR 0005. Files: `src-tauri/src/process/`. Verify: parent+child tree dies; port freed.
- [x] **T3 (P1, CC: ~30min)** — state — Adaptive poll loop; `set_active`/`set_idle` commands; emit `snapshot`; isolate panicking probes.
  - Surfaced by: Finding 3; ADR 0001. Files: `src-tauri/src/state/`. Verify: cadence switches; one probe panic doesn't blank others.
- [x] **T4 (P1, CC: ~30min)** — docker — `bollard` read-only list (running+stopped); cache "not detected" when socket absent.
  - Surfaced by: Section 1. Files: `src-tauri/src/docker/`. Verify: socket-absent path shows "not detected", no crash.
- [x] **T5 (P1, CC: ~45min)** — ui — Tray + positioned popover window (lazy webview), Svelte scaffold, `PortList`/`DockerList`/`StatusBadge`.
  - Surfaced by: Findings 2; ADR 0002. Files: `src/`, `src-tauri/tauri.conf.json`. Verify: first-open spawns webview; idle RAM under budget.
- [x] **T6 (P1, CC: ~15min)** — build — Wire `tauri-specta` to generate TS types for commands + events.
  - Surfaced by: Finding 5. Files: `src-tauri/src/bindings.rs`, build script. Verify: TS types regenerate from Rust.
- [x] **T7 (P2, CC: ~15min)** — ci — GitHub Actions: build + ad-hoc sign + publish to Releases; cut `v0.1-beta` at end of Layer 1.
  - Surfaced by: ADR 0006, 0007. Files: `.github/workflows/`. Verify: workflow publishes a downloadable artifact.
- [x] **T8a (P1, CC: ~30min)** — projects/store — `ProjectStore` + `projects.json`: atomic write (temp + `fs::rename`), `version` field, path canonicalization, dedupe-by-canonical-path, corrupt→backup `.corrupt-<ts>`+empty+notice, deleted-folder tolerance.
  - Surfaced by: L2-9; ADR 0004. Files: `src-tauri/src/projects/store.rs`. Verify: round-trip; corrupt file backed up not clobbered; missing file → empty; duplicate folder → one project.
- [x] **T8b (P1, CC: ~20min)** — projects/parse — Task discovery: `package.json` scripts + `docker-compose.yml`/`compose.yml`/`compose.yaml` services (via `yaml-rust2`) → suggested Tasks.
  - Surfaced by: L2-8; CONTEXT.md Task. Files: `src-tauri/src/projects/parse.rs`, `Cargo.toml` (+`yaml-rust2`). Verify: scripts parsed; compose service names parsed; no-scripts/malformed json/anchors+comments handled.
- [x] **T8c (P1, CC: ~30min)** — projects/spawn — `$SHELL -l -c <cmd>` in cwd, `process_group(0)`, store pgid; piped stdout/stderr → per-process **ring buffer** (~200 lines, capped). No PTY. (`.env` auto-load deferred to Layer 3.)
  - Surfaced by: L2-4; ADR 0003. Files: `src-tauri/src/projects/spawn.rs`. Verify (integration, real procs): valid cmd binds port; portless cmd; exit 0; exit ≠ 0; command-not-found captured in ring buffer.
- [x] **T8d (P1, CC: ~25min)** — projects/lifecycle — State machine `starting/running/running_no_port/exited/crashed` from `try_wait` + pgid-port match + ring-buffer; `running_no_port` neutral + honesty-marked.
  - Surfaced by: L2-3, L2-5, L2-6; ADR 0013. Files: `src-tauri/src/projects/lifecycle.rs`. Verify: each transition; running_no_port after grace; exit codes mapped.
- [x] **T8e (P1, CC: ~25min)** — projects/registry + state — `ProjectRegistry` (`Arc<Mutex>`), `ManagedStatus`; poller reads pgid+`try_wait`, **lock released before Docker await**; emit runtime-only `managed` section in `Snapshot` (definitions stay in commands).
  - Surfaced by: L2-1, L2-2. Files: `src-tauri/src/projects/registry.rs`, `src-tauri/src/state/mod.rs`. Verify: managed status reflected; no `all_snapshots` in hot path; no lock-across-await.
- [x] **T8f (P1, CC: ~20min)** — projects/commands + ui — Commands `load_projects`/`save_project`/`add_task`/`remove_project`/`start_task`/`stop_task`; folder picker (`tauri-plugin-dialog` + capability); `PROJECTS` section + ▶ start in popover.
  - Surfaced by: L2-2, L2-12. Files: `src-tauri/src/projects/`, `src-tauri/capabilities/default.json`, `src/`. Verify: pick folder → project saved; start → status appears; bindings regenerate.
- [x] **T8g (P1, CC: ~20min)** — projects/save-as — Save-as-project: walk parent chain from listener PID (`all_snapshots`, save-time) → candidate commands → user picks/edits + cwd.
  - Surfaced by: L2-10; ADR 0004. Files: `src-tauri/src/projects/`, `src/`. Verify: listener `node` walks up to `pnpm dev` candidate; user edit persists; secrets editable out.
- [x] **T8h (P1, CC: ~20min)** — kill-on-quit — Wire Tauri `RunEvent::Exit`/`ExitRequested` (currently plain `.run()`): `killpg(pgid,SIGTERM)`→bounded grace→`killpg(SIGKILL)` for each Portus-started group; no post-crash adoption.
  - Surfaced by: L2-7; ADR 0013. Files: `src-tauri/src/lib.rs`, `src-tauri/src/projects/`. Verify (integration): managed group dies on quit; re-parented child still dies (pgid); window-hide does NOT kill.
- [ ] **T8-L3 (P1, CC: ~1h)** — logs/pty — **Layer 3** (login-shell `.env` auto-load, `portable-pty`, LogPeek ANSI→HTML, send-input, `waiting?`, `docker logs -f`). Upgrades Portus-started → Managed.
  - Surfaced by: ADR 0003, 0008, 0013. Files: `src-tauri/src/logs/`, `src/LogPeek.svelte`. Verify: start task → live logs stream; answer y/n prompt.
- [ ] **T0 (P1, CC: ~30min)** — spike — Layer 0 throwaway: Tauri app spawns one PTY + streams a line; measure release binary size; confirm argv/cwd recovery for own PIDs.
  - Surfaced by: Codex (early spike); ADR 0002, 0004, 0007. Files: throwaway branch. Verify: go/no-go on PTY + `<15MB`.
- [ ] **T9 (P1, CC: ~20min)** — process — Revalidate identity (start-time + exe, port still held) before kill/stop; build descendant set from the revalidated live tree.
  - Surfaced by: ADR 0009. Files: `src-tauri/src/process/`. Verify: simulate PID reuse → action aborts, list refreshes.
- [ ] **T10 (P1, CC: ~30min)** — docker/ports — Reconcile Docker host-proxy sockets with bollard mapping → single container row; suppress raw-kill on proxy.
  - Surfaced by: ADR 0010. Files: `src-tauri/src/{docker,ports}/`. Verify: published container port shows one row, not two.
- [ ] **T11 (P1, CC: ~30min)** — ports — Normalize into the port model: TCP, v4/v6 dedup, wildcard scope, multi-PID, stable row key.
  - Surfaced by: ADR 0011. Files: `src-tauri/src/ports/`. Verify: dual-stack service = one stable, non-flickering row.
- [ ] **T12 (P2, CC: ~15min)** — process/ui — Privilege-gated action state ("needs elevated privileges") instead of silent EPERM; row stays visible.
  - Surfaced by: ADR 0012. Files: `src-tauri/src/process/`, `src/`. Verify: kill on a root-owned proc shows the gated state.
- [x] **T13 (P1)** — process/logs — Managed lifecycle state machine (incl. `running_no_port`) + kill-on-quit; no post-crash adoption. **Superseded/expanded by T8d (lifecycle) + T8e (registry) + T8h (kill-on-quit)** under the Layer 2 locked decisions (L2-3..L2-7).
  - Surfaced by: ADR 0013. See L2-3..L2-7.
- [ ] **T14 (P2, CC: ~20min)** — state/ui — Poll single-flight (drop overlapping/late Docker calls); `set_idle` via focus/blur + safety timeout.
  - Surfaced by: Codex; ADR 0001. Files: `src-tauri/src/state/`, `src/`. Verify: slow Docker doesn't reorder snapshots; closing popover always returns to idle cadence.

## NOT in scope (deferred, with rationale)

- **Apple notarization + in-app Tauri auto-updater** — $99/yr deferred; `brew upgrade` covers updates (ADR 0006).
- **Full GUI E2E (tauri-driver)** — flaky/high-maintenance for a menubar app; Rust integration + component tests cover the real risk (Finding 6).
- **xterm.js / full terminal** — fights lightweight ADR + positioning guardrail (ADR 0008).
- **SQLite** — JSON is right-sized for saved projects; only reconsider for persistent log history/metrics.
- **Windows / Linux builds** — trait-ready, enabled after macOS ships.
- **Task ordering/dependency engine** — `&&` + docker-compose cover common cases (CONTEXT.md / ADR 0003).
- **Event-driven port discovery** — v2 polling optimization (ADR 0001).
- **Opt-in project auto-detect, Procfile/Makefile/git-crawl** — v1.1.
- **Privileged helper / run-as-root kill** — best-effort only in v1; revisit on real demand (ADR 0012).
- **UDP ports** — TCP only in v1; the port model leaves room (ADR 0011).
- **Post-crash adoption of previously-Managed processes** — they degrade to Detected (ADR 0013).

## Popover UI design (from /plan-design-review)

Classifier: **App UI** (dense, task-focused, glance-and-dismiss). Calm surface,
minimal chrome, one accent, monospace only for ports/PIDs.

**Layout — grouped single scroll.** One scrollable list with sticky section
headers (`PROJECTS`, `PORTS`, `DOCKER`); a pinned **glance header** summary on
top (`3 running · 1 waiting?` + settings ⚙). Nothing hidden behind tabs.

```
┌─ Portus ─────────────────┐
│ 3 running · 1 waiting?  ⚙ │  ← pinned glance target
├─ PROJECTS ───────────────┤
│ web            ▶ start    │
├─ PORTS ──────────────────┤
│ ● :3000  node          ✕ │  ← primary line
│    from IDE · 1.2% · 240MB · ~/web   (dim secondary)
├─ DOCKER ─────────────────┤
│ ● :5432  postgres        │
└──────────────────────────┘
```

**Row anatomy — two-line, dim secondary.** Primary: status glyph + port +
process + kill (on hover). Secondary (dimmed): source chip + CPU/mem + cwd.
Expanding a row opens the inline LogPeek beneath it.

**Status encoding — shape + color (colorblind-safe, WCAG 1.4.1).** Glyph carries
meaning without color: `●` running (green), `◐` waiting? (amber), `○` stopped
(grey), `✕` crashed (red). Header summary uses words too.

**Captured (not yet decided — see TODOs / DESIGN.md):**
- **No DESIGN.md yet** — accent color, type scale, and the monospace face for
  ports/PIDs are unspecified. Recommend `/design-consultation` before frontend
  build so the system is named, not improvised.
- **Loading / first-open state** — first popover open spawns the webview + runs
  the first poll; needs a defined skeleton/loading treatment (not a blank flash).
- **Keyboard navigation** — for a power-dev tool: arrow keys through rows, Enter to
  expand LogPeek, a kill shortcut, visible focus ring. Spec before build or it
  won't exist.

## Known platform work (not a gap — flagged effort)

A Tauri window is **not** automatically a correct macOS menu-bar popover. Budget
real time in Layer 1 for: under-tray positioning, click-outside dismissal, focus
behavior, Spaces/fullscreen-app interaction, multi-display, and activation policy
(no Dock icon). The `ahkohd/tauri-macos-menubar-app-example` (`v2-popover`)
covers the pattern, but this is platform plumbing, not a one-liner.

## What already exists

Nothing in-repo (greenfield). Everything OS-level is provided by mature/maintained
libraries (`listeners`, `sysinfo`, `bollard`, `portable-pty`, Tauri v2 +
positioner). Custom code is limited to: the poller, the lazy tree-kill walk, the
project/PTY glue, and the Svelte UI. We assemble; we do not reinvent port scanning,
process info, or Docker access.

## Open / unresolved

- Container lifecycle control (start/stop via bollard) — listed but not gated;
  decide during Layer 1 whether to include in v1 or defer.
