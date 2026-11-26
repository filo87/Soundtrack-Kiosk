# Soundtrack Kiosk

A single-event macOS kiosk. Guests type their name, pick a song, choose a print style, and walk away with a 10x15 cm album-companion print. The song is appended to the organizer's Spotify playlist; the print is stuck into a physical album, where the guest can handwrite a memory or a wish beside the cover.

Built with Tauri v2, React + TypeScript, and a Rust backend.

## Features

- Keyboard-first guest flow: name -> search -> print style -> done.
- Spotify track search with a manual title/artist fallback when Spotify is unavailable or not authenticated.
- Four print styles: **Portrait/Landscape x Cover/Fit**.
  - **Cover** crops the art to fill the paper; it is full-bleed and leaves no writing margin.
  - **Fit** shows the full art centered on a white background, leaving margins for a handwritten
    memory or wish.
- Cover resolution chain: iTunes 3000x3000 (strict artist/title match) -> Spotify 640 -> bundled placeholder.
- Append-only local journal with a playlist outbox, print retry/reprint, and CSV export.
- Duplicate protection by normalized song title + artist; the second print is still made, but the song is not added to the playlist again.
- CUPS printing for the Canon SELPHY CP1500, with a Mock backend for development.
- Staff status dot (salmon when auth, network, printer, refill, or pending-playlist attention is needed).
- Admin overlay with Overview, Spotify, Printer, Event, Security and History sections.
- Kiosk hardening: borderless fullscreen, always on top, Dock/menu bar hidden, process switching disabled, close/minimize guards.

## How guests use it

1. Enter a name and pick a song; the song is added to the shared Spotify playlist.
2. Choose orientation and whether the album art should fill the paper (**Cover**) or leave a white
   writing margin (**Fit**).
3. Collect the print, stick it into the physical album, and write a memory or a wish beside it.

The app prints the album art and the chosen layout. The memory/wish itself is handwritten on the
paper; it is not typed into the kiosk or stored digitally.

## Palette

The UI uses these five colors consistently:

| Color | Hex | Use |
|---|---|---|
| Sweet Salmon | `#FE938C` | selection, attention dot, accents |
| Desert Sand | `#E6B89C` | surfaces and input fields |
| Wheat | `#EAD2AC` | page background |
| Cool Steel | `#9CAFB7` | borders and preview wells |
| Cerulean | `#4281A4` | selection rings, focus borders, links |

Body text uses a dark blue ink (`#223843`) and muted secondary text (`#40535E`) for contrast.

## Requirements

- macOS 11 or newer (the app is macOS-only).
- Node.js 20+ and pnpm 12 (`packageManager` is pinned in `package.json`).
- rustup with the toolchain pinned in `rust-toolchain.toml` (`1.100.0`), including `rustfmt` and `clippy`.
- Xcode Command Line Tools: `xcode-select --install`.
- CUPS tools (`lp`, `lpstat`) for real printing.
- A Spotify developer app:
  - The app owner needs Spotify Premium (Spotify Development Mode requirement).
  - Development Mode allows at most 5 allowlisted users.
  - Redirect URI must be exactly `http://127.0.0.1:43821/callback`.
  - Requested scopes: `playlist-modify-public playlist-modify-private playlist-read-private`.

## Quick start

```bash
pnpm install --frozen-lockfile
pnpm tauri dev
```

The first Rust build takes several minutes. The window is a real kiosk window: borderless, always on top, and fullscreen.

- Exit: `Ctrl + Option + Shift + K`, PIN `0000`, then **Exit kiosk** (or force-quit).
- Debug builds use the **Mock printer** and copy simulated prints to `mock-prints/`.
- To exercise CUPS in development: `BABY_KIOSK_PRINTER=cups pnpm tauri dev`.
- `pnpm dev` alone only serves the React UI in a browser; Tauri IPC calls will fail. Use it for layout work only.

Production bundle:

```bash
pnpm tauri build
open "src-tauri/target/release/bundle/macos/Soundtrack Kiosk.app"
```

Debug bundle:

```bash
pnpm tauri build --debug --bundles app
open "src-tauri/target/debug/bundle/macos/Soundtrack Kiosk.app"
```

## Spotify setup

1. In the Spotify Developer Dashboard, create an app and add the redirect URI above.
2. Start the kiosk, open admin (`Ctrl + Option + Shift + K`), and set the **Spotify client id** and **playlist URL**.
3. Save settings, then click **Login / relogin**. The browser opens; the kiosk temporarily leaves kiosk mode for the login.
4. The playlist owner must match the logged-in account. The owner check runs when settings are saved.

Without a valid login, search returns no results and the guest flow falls back to manual song/artist entry. Those selections are journaled, printed, and added to the playlist once login returns.

## Admin

Open with `Ctrl + Option + Shift + K`.

The overlay is split into six sections:

- **Overview** — what needs attention, paper/ink progress, refill actions, and a test print.
- **Spotify** — connection, client id, playlist URL with a parsed playlist id preview, market and iTunes country.
- **Printer** — backend badge, queue selection, default orientation, status/reason, and a test print.
- **Event** — baby name and welcome line with a live Name-screen preview.
- **Security** — change PIN, maintenance mode, and the kiosk exit danger zone.
- **History** — searchable recent selections, per-row reprint, CSV export, and local history clear.

Fresh install PIN: `0000`. The kiosk shows a salmon staff dot and refuses to save settings until a
different 4-12 digit PIN is set.

- Unsaved edits show a sticky **Unsaved changes** bar. **Save** sends the whole settings form;
  **Discard** restores the last saved values. Paper and ink counters are server-owned and update
  automatically. `Cmd/Ctrl+S` saves.
- The **Spotify** section shows the redirect URI with a **Copy** button. It must match the Spotify
  app character for character; the callback port is fixed at 43821.
- Admin sessions expire after 5 minutes idle; Esc closes the overlay. Closing with unsaved edits
  asks for confirmation.
- **Clear local history** (History section) requires typing `CLEAR`. It archives the journal to
  `journal-YYYYMMDD-HHMMSS-mmm.jsonl`, empties the local list, clears local duplicate memory, and
  drops pending playlist adds (archived, not sent). The Spotify playlist is not changed. Print
  files are kept.
- **Reset both** zeroes the paper and ink counters. **Paper refilled** and **Ink replaced** reset
  one counter at a time.
- **Export CSV** (History section) writes `~/Desktop/soundtrack-YYYYMMDD-HHMMSS.csv`.
- **Maintenance mode** (Security section) temporarily leaves kiosk mode for browser or diagnostics
  work.

## Printing

The printer is a Canon SELPHY CP1500 over USB through CUPS.

- Debug/default backend: `Mock` (no hardware needed).
- Release/default backend: `Cups`.
- Override either with `BABY_KIOSK_PRINTER=mock` or `BABY_KIOSK_PRINTER=cups`.
- The app submits portrait/landscape PNGs with `lp`, `media=Postcard`, `print-scaling=fill`, and `orientation-requested=3|4`.
- Media and borderless options must be confirmed on the event unit. Record the frozen values in `docs/printer-notes.md`.

## Data

App data directory:

```text
~/Library/Application Support/com.soundtrack.kiosk/
  settings.json             # 0600; includes the plaintext admin PIN
  tokens.json               # 0600; Spotify access/refresh tokens
  journal.jsonl             # append-only selection/print/playlist history
  covers/                   # cover cache, one directory per album/manual key
  previews/                 # transient four-layout previews, purged on startup
  prints/YYYY-MM-DD/        # durable print files referenced by the journal
  mock-prints/              # Mock backend output
  journal-*.jsonl           # archives created by Clear local history
```

Logs are written by `tauri-plugin-log` under:

```text
~/Library/Logs/com.soundtrack.kiosk/
```

`journal.jsonl` is the source of truth. Deleting or truncating it manually is not supported; use the admin action so the file is archived and the in-memory projection is reset.

## Upgrading from Baby Playlist Kiosk

The previous build used the bundle identifier `com.soundtrack.babykiosk`. Soundtrack Kiosk uses
`com.soundtrack.kiosk`.

On first launch, if the new data directory has no settings/tokens/journal, the app copies the old
data directory (settings, tokens, journal, covers, prints) into the new one. The old directory is
left untouched as a backup.

To replace an installed build:

1. Quit the old app.
2. Replace `/Applications/Baby Playlist Kiosk.app` with `Soundtrack Kiosk.app`.
3. Launch Soundtrack Kiosk once and confirm settings, login, and history.
4. Remove the old app and the old data directory when satisfied.
5. Reinstall the LaunchAgent from `src-tauri/launchd/com.soundtrack.kiosk.plist`; unload the old
   `com.soundtrack.babykiosk` label first.

Old logs remain under `~/Library/Logs/com.soundtrack.babykiosk/`.

## Scripts

| Command | Purpose |
|---|---|
| `pnpm dev` | Vite only (no Tauri IPC). |
| `pnpm build` | Typecheck + production frontend build. |
| `pnpm tauri dev` | Full app in development (Mock printer by default). |
| `pnpm tauri build` | Release `.app`/`.dmg` (CUPS by default). |
| `pnpm test` | Vitest run. |
| `pnpm test:watch` | Vitest watch mode. |
| `pnpm typecheck` | `tsc --noEmit`. |
| `pnpm lint` | ESLint. |
| `pnpm rust:test` | `cargo test`. |
| `pnpm rust:clippy` | `cargo clippy --all-targets -- -D warnings`. |
| `pnpm verify` | Typecheck, lint, Vitest, rustfmt check, clippy, Rust tests. |

`pnpm verify` needs cargo on PATH. If `cargo` is not found, add the rustup shim:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

## Testing

```bash
pnpm verify
```

Current coverage:

- Vitest: reducer transitions, idle timers, stale search responses, full guest cycle and manual fallback, four-layout commit, command ACL coverage.
- Rust: PKCE vector, token rotation/refresh/concurrency, callback parsing, journal projection/recovery/clear, cover matching and iTunes URL rewrite, four render variants, CUPS parsers, duplicate identity.

Hardware/manual gates that cannot be automated here:

- Real Spotify login and playlist insert.
- CP1500 paper/orientation/borderless matrix and printer error drills.
- LaunchAgent/auto-login installation.
- Offline drill: Wi-Fi off -> manual entry -> print -> Wi-Fi on -> outbox drains.

## Architecture

```text
src/ (React)
  App.tsx, main.tsx
  kiosk/machine.ts          pure reducer
  kiosk/useKiosk.ts         effects: search debounce, prepare, commit, idle
  kiosk/screens/            Name, Search, Choose (4 layouts), Done
  admin/                    PIN pad + lazy admin overlay
  lib/ipc.ts                typed invoke wrappers and KioskError
  lib/types.ts              hand-written DTOs mirroring Rust

src-tauri/src/ (Rust)
  lib.rs                    builder, plugins, handlers, window/run guards
  commands.rs               Tauri commands and orchestration
  spotify/                  PKCE auth, token manager, Web API wrapper
  cover/                    iTunes matching + Spotify/placeholder fallback
  render.rs                 four preview variants (cover/fit)
  journal.rs                append-only history + projections
  outbox.rs                 playlist insert retry loop
  print.rs                  CUPS/Mock submission and status
  health.rs                 health event + staff dot data
  kiosk/                    macOS window/presentation options
  state.rs, config.rs, paths.rs, admin.rs, error.rs
```

The webview only talks to Rust through typed Tauri commands. Spotify tokens and all HTTP requests stay in Rust.

## Documentation

- `.kilo/plans/1795620449114-baby-playlist-kiosk-plan.md` - decisions, contracts, milestones, failure modes.
- `docs/runbook.md` - one-page staff runbook.
- `docs/printer-notes.md` - CUPS/SELPHY options and hardware test matrix.
- `AGENTS.md` - repository conventions and change recipes for coding agents.

## Known limitations

- macOS only.
- Physical keyboard required; no on-screen keyboard.
- `Fit` is the writing-friendly layout: it adds a white margin around the full cover. `Cover` is
  full-bleed and leaves no space for a handwritten note.
- The handwritten memory/wish is not captured by the app.
- Duplicate protection is local. Clearing local history resets it, so a song already in the playlist can be added again afterwards.
- Print media/borderless options are provisional until verified on the event printer.
