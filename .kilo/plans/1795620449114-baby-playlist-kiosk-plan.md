# Baby Playlist Kiosk — Implementation Plan (v2, council-reviewed)

A Tauri v2 + React kiosk for one event. Guests type their name and pick a song. The song goes into the organizer's Spotify playlist and a line is written to a local log. The guest then chooses a portrait or landscape print in cover or fit style, and the 10x15 cm album-companion print is sent silently to a Canon SELPHY CP1500. The print is stuck into a physical album where the guest can write a memory or a wish; **fit** leaves the white margin for that note.

This file supersedes `docs/implementation-plan.md`. That document predates this review and still contains effort estimates; M0 aligns it.

---

## 0. Council Review & Sign-off

| Expert | Verdict | Key changes applied |
|---|---|---|
| Simplicity specialist | **Signed off after changes** | Cut MusicBrainz/CAA, iTunes *search* fallback, app-level print queue, per-job print polling, universal binary, `png`/`uuid` crates, disk-space checks, PIN hashing, admin toggles. One degraded path only (manual entry). |
| Minimalist UI/UX designer | **Signed off after changes** | Choosing an orientation *is* the print action (one decision, one keypress). Keyboard-first flow. Nothing is committed until the print is chosen, so Esc always goes back cleanly. Guest-facing banners and status dot removed. Bundled font, calm palette, restrained motion. |
| Rust architect | **Signed off after changes** | Simple fullscreen instead of native, with macOS presentation options via `objc2-app-kit`. Custom app menu replaces webview key blocking. Maintenance mode for OAuth/admin. Printer is an enum, not `dyn` async trait. launchd `SuccessfulExit=false`. Dependencies optimized in dev profile. |
| Senior TS architect | **Signed off after changes** | Pure discriminated-union reducer plus one effects hook. Typed `ipc.ts` with a `KioskError` code union. `mockIPC` tests. Admin is a lazy overlay in the same window. Strict tsconfig. |

**Blocking issues found and fixed**
1. **Orientation chosen after the print was already queued.** v1 queued the print in `submit_selection`, but the guest picked orientation afterwards. The "second half" of submit was also never specified. → Split into two phases: `prepare_selection` (no side effects) and `commit_selection` (journal + playlist + print).
2. **Always-on-top fullscreen kiosk hides the admin window and the OAuth browser.** → Admin becomes an in-window overlay. Spotify login runs in **maintenance mode**, which temporarily drops always-on-top and the presentation options.
3. **Contradiction: `fullscreen:true` (native macOS fullscreen) vs "prefer simple fullscreen".** Native fullscreen can be exited with Esc, the green button or Ctrl+Cmd+F, and it misbehaves with always-on-top. → No native fullscreen. The kiosk uses a borderless window sized to the display, with presentation options hiding the Dock and menu bar.
4. **Cmd+Q/H/M/W and Ctrl+Cmd+F are AppKit menu shortcuts, so webview `preventDefault` cannot block them.** Simply removing the menu would also break Cmd+V/Cmd+A in inputs. → Use a custom menu with an Edit menu only, plus close/exit guards.
5. **`RunEvent::Resumed` is mobile-only.** → Use `Ready`, `Focused` and `ScaleFactorChanged`.
6. **launchd `KeepAlive=true` would relaunch the app after an intentional admin exit.** → Use `KeepAlive { SuccessfulExit = false }`.
7. **`async fn` in a trait is not dyn-compatible.** → `enum PrinterBackend { Cups, Mock }`.
8. **Fonts must work offline.** → Bundle woff2 files and add `font-src 'self'` to the CSP.
9. **Lanczos resize of 3000px covers is very slow in debug builds.** → Set `[profile.dev.package."*"] opt-level = 3`.

---

## 1. Locked Decisions

| Area | Decision |
|---|---|
| Printer | Canon SELPHY **CP1500 over USB**, driven through CUPS `lp`. A `Mock` backend is used for development and tests. |
| Host OS | **macOS**, on a dedicated `kiosk` user with auto-login and a LaunchAgent. |
| Window | **Kiosk fullscreen** (see M1): borderless, sized to the primary display, always on top, Dock and menu bar hidden, process switching disabled in guest mode, re-asserted on focus, display change and relaunch. |
| Input | **Physical keyboard** (a mouse also works). No on-screen keyboard. |
| Orientation | Guest sees **both crops side by side** and picks one. Picking prints. The admin sets which one is preselected. |
| Ownership | Rust owns network, tokens, files, rendering and printing. The webview loads only bundled assets and never sees tokens. |
| Persistence | Append-only `journal.jsonl` (fsync per line) is the source of truth. The outbox and the unprinted list are derived from it at startup. Settings and tokens use `plugin-store` (file, 0600). |
| Admin | In-window overlay opened by a keyboard chord plus PIN. Every admin command is guarded in Rust by an `AdminSession` that expires after 5 min idle. |
| Cover chain | iTunes (strict match, `3000x3000bb`) → Spotify 640px → bundled placeholder. |
| Degraded mode | If Spotify search is unavailable (offline, auth or quota problem), the guest types the title and artist manually. The selection is journaled and printed, and the outbox resolves the URI later. |

### Spotify constraints (verified 2026-11-25, re-check at T-7d)
- Playlist insert uses `POST /v1/playlists/{id}/items`; `/tracks` no longer exists.
- Search `limit` max is 10. Use `limit=8`.
- Dev Mode requires the app owner to have Premium, allows at most 5 allowlisted users, and needs a loopback redirect `http://127.0.0.1:43821/callback`.
- A `429` with `"reason":"QUOTA_EXCEEDED"` is a quota error; a `429` with `Retry-After` is a rate limit.

## 2. Defaults (change in admin/config if needed)

- Duplicate song (normalized title+artist) or URI: still journaled and printed; the playlist insert is skipped and recorded as `playlist_skipped`.
- Print layouts: **cover** crops to fill the paper (no writing margin); **fit** shows the full album art centered on a white background, leaving a margin for the handwritten memory/wish.
- No explicit-content filter.
- Spotify `market` and iTunes `country` come from config (the organizer's country).
- Spotify 640px is the last-resort cover. Cropping it is an accepted policy risk for a private event.
- Copy is configurable (`babyName`, welcome line). Default printer is the CUPS default destination.
- PIN is a plain value in settings. The settings file is readable only by the `kiosk` user. Rust enforces a lockout of 5 wrong attempts → 60 s.

---

## 3. Guest Flow Spec (keyboard-first)

Global rules: input focus is always on the screen's field; the mouse can hover and click; Esc goes back one step; there are no guest-facing error banners.

| Screen | Content | Keys / actions | Timers |
|---|---|---|---|
| **1 Name** | "A song for {babyName}", "What's your name?", large input, subtle "Enter ↵" hint | Type; Enter continues if the trimmed name is 1–40 chars; Esc clears | Typed text cleared after 60 s idle |
| **2 Search** | "Hi {name}, which song should she hear?", search field, up to 8 rows (64px cover, title, artist), first row highlighted | Type (250 ms debounce, ≥2 chars); ↑/↓ moves the highlight and prefetches that cover; Enter or click picks; Esc clears the query, a second Esc returns to Name. No results: "No songs found, try the artist's name". Spotify unavailable: the screen switches inline to a **manual entry** form (Song, Artist) and Enter picks | Back to an empty Name after 60 s idle |
| **3a Choose print** | "Choose your print", four previews (portrait/landscape × cover/fit) showing the exact files that will print, default portrait + cover preselected | ←/→ selects orientation; ↑/↓ selects cover (full-bleed crop) or fit (full art on white, leaving a margin for a handwritten memory/wish); Enter or click **commits and prints**; Esc returns to Search with nothing saved. While previews render, show the small cover with a soft pulse | Auto-commits the preselected option after 30 s idle (the song was already chosen) |
| **3b Done** | Chosen print, "Thank you, {name}! Your print is on its way, collect it at the printer in about a minute." If prints are waiting: "…after {n} other print(s)". | Enter → Name | Back to Name after 12 s; the next guest never waits for the printer |

**Staff indicator:** a 10px dot in the top-right corner. It is invisible when everything is healthy and turns amber when attention is needed: auth lost, printer error, outbox items pending for more than 5 min, or a refill due.

**Admin:** `Ctrl+Option+Shift+K` → PIN overlay → admin overlay. The admin overlay contains:
- Status: auth, network, printer state, prints waiting, outbox pending, refill counters.
- Spotify login/relogin.
- Playlist URL.
- Printer selection and test print.
- Default orientation and copy text.
- Last 30 selections with Reprint.
- Export CSV to `~/Desktop`.
- Clear local history (archives and empties the journal; the Spotify playlist is untouched).
- "Paper refilled" and "Ink replaced" buttons.
- Maintenance mode.
- Exit kiosk.

**Design tokens** (Tailwind v4 `@theme` in `index.css`):

| Token | Values |
|---|---|
| Colours | bg `#FBF8F4`, ink `#1E1B18`, muted `#6B645C`, accent `#C97B7B` (focus ring 3px). Contrast: ink ≥ 12:1, muted ≥ 4.5:1. |
| Type | One bundled OFL variable sans (woff2). Scale 20 / 28 / 40 / 64 px. |
| Spacing | 8px grid; content max-width 960px, centered. |
| Motion | 200 ms opacity/translate crossfade between screens and a preview fade-in only (framer-motion `AnimatePresence`). Disabled under `prefers-reduced-motion`. |
| Icons | lucide: search, arrow-left, check, printer. |

---

## 4. Backend Contract

**Commands.** All DTOs are `camelCase`. Errors serialize as `{code, message}` with these codes: `AUTH_REQUIRED | OFFLINE | QUOTA | RATE_LIMITED | NOT_FOUND | PRINT_FAILED | ADMIN_LOCKED | INVALID_INPUT | INTERNAL`.

| Command | Behaviour |
|---|---|
| `search_tracks(query)` | Returns `{ tracks: TrackDto[], degraded: bool }`. `degraded` tells the UI to show manual entry. |
| `prefetch_cover(track)` | Fire-and-forget warm-up of the cover cache. |
| `prepare_selection(input: Track \| Manual{title,artist})` | Returns `{ prepId, previews: {portraitCover, portraitFit, landscapeCover, landscapeFit}, coverSource, coverPx }`. Resolves the cover (deadline 6 s), then renders all four print variants into `previews/`. **No journal writes, no playlist calls.** |
| `commit_selection({ id, prepId, guestName, orientation, layout })` | Returns `{ printsAhead, printQueued }`. Idempotent on the client `id` (`crypto.randomUUID()`). Steps run in the order below. A print failure never fails the call; it returns `printQueued:false` and the staff dot turns amber. |
| Admin | `admin_unlock(pin)`, `admin_lock`, `admin_status`, `auth_start`, `auth_logout`, `get_config`, `set_config`, `list_printers`, `test_print`, `list_recent`, `reprint(id)`, `export_csv`, `clear_history`, `mark_refilled(kind)`, `reset_printer_counters`, `set_maintenance(bool)`, `exit_kiosk`. Each checks `AdminSession`. |

`commit_selection` steps:
1. Append a `selection` line to the journal (fsync).
2. Move the chosen preview file to `prints/`.
3. Submit the print with `lp`.
4. Append a `print_sent` or `print_failed` line.
5. Call `outbox.notify()`.

**Event:** `kiosk://health`, emitted on change:
```
{ auth, network, printer: {state, reason}, printsWaiting, outboxPending, refillDue, attention }
```

**`AppState`** (`app.manage`): `config: RwLock<Config>`, `http: reqwest::Client`, `tokens: TokenManager`, `journal: Journal(Mutex<File>)`, `printer: PrinterBackend`, `admin: AdminSession`, `kiosk: KioskFlags { maintenance, exit_allowed }`.

**Journal lines.** Each line is `{v:1, event, id, timestamp (RFC 3339 local), ...}`.
- `selection` adds `guest_name, track_title, artist, track_uri|null, orientation, layout: "cover"|"fit", cover_source, cover_px, mode: "spotify"|"manual"`.
- `playlist_added` adds `uri`.
- `print_sent` adds `file, job`.
- `print_failed` adds `reason`.

**Data dir** (`app_data_dir`): `settings.json`, `tokens.json`, `journal.jsonl`, `covers/`, `previews/` (purged on startup), `prints/YYYY-MM-DD/`, `logs/`.

---

## 5. Repository Layout

```
rust-toolchain.toml  package.json  vite.config.ts  tsconfig.json  eslint.config.js
src/
  main.tsx  App.tsx  index.css            # @import "tailwindcss"; @theme; @font-face
  assets/fonts/*.woff2  assets/placeholder.jpg
  lib/{ipc.ts, types.ts, useIdle.ts, useHealth.ts}
  kiosk/{machine.ts, useKiosk.ts, TrackList.tsx, ManualEntry.tsx}
  kiosk/screens/{NameScreen,SearchScreen,ChooseScreen,DoneScreen}.tsx
  admin/{AdminOverlay.tsx, PinPad.tsx}    # React.lazy
  components/StaffDot.tsx
src-tauri/
  Cargo.toml  build.rs  tauri.conf.json  capabilities/kiosk.json
  resources/placeholder.jpg  launchd/com.<org>.babykiosk.plist
  src/
    main.rs lib.rs state.rs error.rs config.rs paths.rs commands.rs
    kiosk/{mod.rs, macos.rs}              # window, menu, presentation options, guards, maintenance
    spotify/{pkce.rs, auth.rs, token.rs, api.rs}
    cover/{mod.rs, itunes.rs, matching.rs}
    render.rs print.rs journal.rs outbox.rs health.rs admin.rs
docs/printer-notes.md
```

---

## 6. Task List

Dependencies:
- M0 → M1, M2, M4
- M2 → M3
- M4 → M5
- M3 and M5 → M6. The UI can start earlier against `mockIPC`.
- M6 → M7 → M8

### M0 — Toolchain, scaffold, quality gates
- [ ] Install rustup (not Homebrew Rust): `rustup default stable`, `rustup component add rustfmt clippy`, `xcode-select --install`, `cargo install tauri-cli --version "^2" --locked`, `cargo install create-tauri-app --locked`.
- [ ] Add `rust-toolchain.toml` pinning `channel` to the installed stable version, with `components = ["rustfmt","clippy"]` and `profile = "minimal"`. Build only for the kiosk Mac's native architecture.
- [ ] Scaffold with `cargo create-tauri-app` (template `react-ts`, `npm`) into the repo root. Identifier: `com.<org>.babykiosk`.
- [ ] Frontend dependencies: `@tauri-apps/api`, `tailwindcss`, `@tailwindcss/vite`, `framer-motion`, `lucide-react`. Dev: `vitest`, `jsdom`, `@testing-library/react`, `typescript-eslint`, `eslint-plugin-react-hooks`, `prettier`. tsconfig: `strict`, `noUncheckedIndexedAccess`. No `@tauri-apps/plugin-*` JS packages.
- [ ] Rust dependencies via `cargo add`:
  - Tauri: `tauri` (`protocol-asset`), `tauri-plugin-store`, `tauri-plugin-opener`, `tauri-plugin-log`, `tauri-plugin-single-instance`.
  - General: `reqwest` (`json`, rustls, `default-features=false`), `tokio` (`net,process,time,sync,fs,macros`), `serde`, `serde_json`, `thiserror`, `chrono`, `url`.
  - PKCE: `base64`, `sha2`, `rand`.
  - Images and matching: `image` (`jpeg,png,webp`, no defaults), `unicode-normalization`.
  - macOS only (`[target.'cfg(target_os="macos")']`): `objc2`, `objc2-foundation`, `objc2-app-kit`.
  - Dev: `wiremock`, `tempfile`.
- [ ] Profiles:
  - Release: `lto=true`, `codegen-units=1`, `opt-level="s"`, `strip=true`, `panic="unwind"`.
  - Dev: `[profile.dev.package."*"] opt-level=3`.
- [ ] `tauri.conf.json`:
  - Window `main`: `fullscreen:false`, `decorations:false`, `resizable:false`, `alwaysOnTop:true`, `visible:false`, `dragDropEnabled:false`.
  - CSP: `default-src 'self'; img-src 'self' asset: http://asset.localhost data: https://i.scdn.co; font-src 'self'; style-src 'self' 'unsafe-inline'; connect-src ipc: http://ipc.localhost`.
  - `assetProtocol` enabled with scope `["$APPDATA/previews/**","$APPDATA/prints/**"]`.
- [ ] `build.rs`: use `AppManifest::commands(&[...])`. `capabilities/kiosk.json` lists `core:default` plus all app commands; authorization is enforced in Rust (`AdminSession`).
- [ ] `error.rs` (`AppError` with manual `Serialize` to `{code,message}`) and `state.rs`. Wrap the access and refresh tokens in a `Secret(String)` newtype whose `Debug` prints `[redacted]`.
- [ ] Add an `npm run verify` script that runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `tsc --noEmit`, `eslint`, `vitest run`. GitHub Actions is optional.
- [ ] Replace `docs/implementation-plan.md` with a short pointer to this plan, or align it, and remove the effort estimates.

Validate: `cargo tauri dev` launches and a `ping` command round-trips; `npm run verify` passes.

### M1 — Kiosk window & fullscreen (`kiosk/`)
- [ ] In `setup`:
  1. Read `primary_monitor()`.
  2. `set_position`/`set_size` with **physical** units to the monitor bounds.
  3. Apply kiosk presentation options on the main thread (`run_on_main_thread`): `HideDock | HideMenuBar | DisableAppleMenu | DisableProcessSwitching | DisableHideApplication | DisableMenuBarTransparency`. Leave force-quit enabled as an emergency escape; launchd relaunches the app.
  4. `show()` + `set_focus()`.
  5. If the pinned Tauri exposes macOS `set_simple_fullscreen`, use it instead of manual sizing.
- [ ] Custom app menu (`Menu::with_items`): an **Edit** menu only (Undo, Redo, Cut, Copy, Paste, Select All), so text shortcuts work. No Quit, Hide, Minimize, Close or Full Screen items.
- [ ] Guards:
  - `WindowEvent::CloseRequested` → `prevent_close()` unless `exit_allowed`.
  - `RunEvent::ExitRequested` → `prevent_exit()` unless `exit_allowed`.
  - On `Focused(true)`, `Moved`, `Resized` and `ScaleFactorChanged`: when not in maintenance mode, re-apply size, position and presentation options **only if the bounds differ** (avoids resize loops).
- [ ] Maintenance mode: `set_maintenance(true)` (from admin, and automatically during `auth_start`) turns off always-on-top, restores normal presentation options and pauses the guards. `set_maintenance(false)`, login completion or a 5-min timeout restores kiosk mode.
- [ ] Webview hardening: `user-select:none` except inputs, `contextmenu` prevented, no zoom, no drag. `exit_kiosk` sets `exit_allowed` and calls `app.exit(0)`.

Validate: the window covers the display with no menu bar and no Dock. Cmd+Q, Cmd+W, Cmd+H, Cmd+M, Ctrl+Cmd+F, Cmd+Tab and right-click have no effect. Cmd+C, Cmd+V and Cmd+A work in inputs. Unplugging and replugging the display, or changing resolution, restores fullscreen. Maintenance mode lets the browser appear on top, and leaving it restores the kiosk.

### M2 — Spotify PKCE auth & tokens (`spotify/`)
- [ ] `pkce.rs`: verifier of 64 random bytes in base64url without padding; `challenge = base64url(sha256)`; random `state`.
- [ ] `auth.rs`:
  1. Bind `std::net::TcpListener` on `127.0.0.1:43821`, then convert with `tokio::net::TcpListener::from_std` (non-blocking) and wrap in `tokio::time::timeout(5 min)`.
  2. Open the authorize URL with `opener`, using scopes `playlist-modify-public playlist-modify-private playlist-read-private`.
  3. Ignore requests other than `GET /callback`. Validate `state` and handle `error=`. Reply with a "Done, you can close this tab" page.
  4. Exchange the code with a form-encoded `POST /api/token`.
- [ ] `token.rs`: `TokenManager` around `tokio::sync::Mutex<Tokens>`.
  - Refresh when `expires_at - 60s < now`.
  - **Persist a rotated `refresh_token` before using the new access token**; keep the old one if the response has none.
  - `invalid_grant` → `NeedsLogin`.
- [ ] `api.rs` request wrapper: one retry on 401; on 429 honour `Retry-After` (cap 10 s) or return `QUOTA`; two jittered retries on 5xx.
- [ ] `auth_start`, `auth_logout`, playlist URL → ID parsing, and an owner check (`GET /v1/playlists/{id}` owner == `GET /v1/me`).

Validate:
- Tests: the RFC 7636 Appendix B vector; wiremock covering rotation, no rotation, `invalid_grant`, and 10 concurrent callers causing exactly 1 refresh; callback parser cases (wrong state, `access_denied`, favicon).
- Manual: a real login survives a restart; a forced-expired token refreshes silently; revoking access turns the staff dot amber with no guest-facing error; tokens never appear in logs.

### M3 — Search, journal, outbox
- [ ] `api.rs`: `GET /v1/search?type=track&limit=8&market=…` → `TrackDto {id, uri, title, artists, album, image640, image64}`. `AUTH_REQUIRED`, `QUOTA` and `OFFLINE` all map to `degraded:true`.
- [ ] `journal.rs`:
  - `Mutex<File>` in append mode; `write_all(line + "\n")` then `sync_data()` inside `spawn_blocking`.
  - `load()` folds by `id` and skips a truncated last line, logging a warning.
  - Projections: `pending_playlist()` and `unprinted()`.
- [ ] `outbox.rs`: a single task that wakes on `notify` or every 30 s.
  - Resolve `track_uri:null` entries with `q=track:"t" artist:"a"` and `limit=1`.
  - Skip URIs that were already added.
  - Batch `POST /v1/playlists/{id}/items` (≤100 URIs) and append one `playlist_added` per selection.
  - On `QUOTA` pause 15 min; on `NeedsLogin` pause until login.
- [ ] `commit_selection` journal and outbox steps, with idempotency on `id`.

Validate:
- Online: the track is in the playlist within 5 s.
- Wi-Fi off: the journal line is written immediately; after reconnecting it drains within 30 s.
- `kill -9` mid-flow: nothing is lost and pending items replay.
- Every selection has `guest_name, track_title, artist, track_uri, timestamp`. `track_uri` is null only in manual mode until resolved.
- Repeating a commit with the same `id` is a no-op.

### M4 — Cover chain & render (`cover/`, `render.rs`)
- [ ] `itunes.rs`: `search?term={artist} {title}&entity=song&limit=10&country=…`, then rewrite `/\d+x\d+bb\.(jpg|png|webp)$/` → `3000x3000bb.$1`.
- [ ] `matching.rs`: NFKD, strip diacritics, lowercase, remove `(feat…)`, `[…]`, and `- Remaster…`/`- Live…` suffixes. Require an artist match **and** a title match (equal, or one is a prefix of the other). Prefer a `collectionName` that matches the album.
- [ ] `cover/mod.rs`:
  - Cache in `covers/{key}.{ext}`, keyed by Spotify album id, or by a normalized artist+title hash in manual mode.
  - 6 s overall deadline. Accept iTunes only if `min(w,h) ≥ 1000` and the match is strict; otherwise use Spotify 640, then the placeholder.
- [ ] `render.rs`: for each orientation, render a cover variant (`resize_to_fill`, centered crop) and a fit variant (contain on a white 1200x1800 / 1800x1200 canvas): `previews/{prepId}-{portrait|landscape}-{cover|fit}.png`. One decode, one blocking task.
- [ ] `prepare_selection` and `prefetch_cover` commands.

Validate:
- Unit tests: URL rewrite, a matching table (remasters, feat., compilations, non-Latin titles), a wrong-artist fixture falling through to Spotify, a deadline fallback, exact output dimensions, and a symmetric-crop golden test on a synthetic image with coloured edges.
- 30-track reference list: ≥80% resolve to ≥1000px and **0** resolve to a wrong album.
- With prefetch, `prepare_selection` p95 ≤ 2 s.
- Offline: a cached cover is reused; with no cache, the placeholder is used without error.
- If the ≥80% target fails, raise it with the user before adding another source.

### M5 — Printing (`print.rs`, `health.rs`)
- [ ] `PrinterBackend::Cups` submits `lp -d <printer> -o media=<frozen> -o print-scaling=fill [-o orientation-requested=<frozen>] <file>` with `tokio::process::Command` and a 15 s timeout, and parses `request id is (\S+)`.
- [ ] `PrinterBackend::Mock` copies the file to `mock-prints/` and reports itself busy for 45 s.
- [ ] `health.rs`, every 10 s:
  - `lpstat -p <printer>` for state and reason.
  - `lpstat -o <printer>` count for `printsWaiting`.
  - Auth state from `TokenManager`.
  - `network` from the last outbound result, with a probe every 30 s only while degraded.
  - Emit `kiosk://health` on change.
- [ ] Refill counters, persisted: paper reminder at 16 prints since the last refill (tray holds 18); ink reminder at 34 since the last replacement (36 per cassette).
- [ ] `test_print`, `reprint(id)` (from the file in `prints/`) and `list_printers` (`lpstat -e`, default from `lpstat -d`).
- [ ] As soon as the hardware arrives, run `lpoptions -p <printer> -l` and record the media, borderless and orientation options in `docs/printer-notes.md`.

Validate: `lp` output parser tests pass; the Mock backend completes the full flow; one real CP1500 print is borderless, correctly oriented and not letterboxed; the printed file is byte-identical to the chosen preview.

### M6 — Kiosk UI & admin overlay (`src/`)
- [ ] `lib/types.ts` mirrors the Rust DTOs by hand. `lib/ipc.ts` has one typed wrapper per command, and parses errors into a `KioskError` with a `code` union. Nothing is typed `any`.
- [ ] `kiosk/machine.ts`: a pure reducer over
  `{screen:'name'} | {screen:'search', guestName} | {screen:'choose', guestName, selection, prep?} | {screen:'done', guestName, printUrl, printsAhead}`,
  plus actions. `useKiosk.ts` handles the effects: IPC calls, a request sequence guard for search, preparing on pick, committing on choose, and generating the client `id` once per choose screen.
- [ ] `lib/useIdle.ts` uses one window-level activity listener and applies the per-screen timeouts from §3. `useHealth.ts` subscribes with `listen` and cleans up, so it is safe under StrictMode.
- [ ] Build the screens exactly as specified in §3. Preview images use `convertFileSrc`. Double-commit is prevented both by reducer state and by Rust idempotency.
- [ ] Admin: a window keydown handler catches the chord → `PinPad` → `admin_unlock`. `AdminOverlay` is lazy-loaded and has the contents listed in §3. Any admin error with code `ADMIN_LOCKED` closes the overlay.
- [ ] Bundle fonts and add the tokens to `index.css`. No external network requests from the webview other than the Spotify thumbnails (`i.scdn.co`).

Validate:
- Vitest covers every reducer transition, the idle timeouts (fake timers) and the search stale-response guard. RTL plus `@tauri-apps/api/mocks` `mockIPC` covers the full cycle and manual entry.
- Manual:
  - The full keyboard-only cycle needs ≤ 6 keypresses after typing; four print options are visible and each commits the preview shown.
  - Esc on Choose saves nothing.
  - Walking away on any screen returns to an empty Name screen.
  - Pressing Enter 10 times quickly on Choose produces 1 journal entry, 1 playlist insert and 1 print.

### M7 — CP1500 hardware validation & soak
- [ ] USB connection.
- [ ] Use `lpoptions -p <printer> -o …` to make Postcard/Borderless the default.
- [ ] Run `lpadmin -p <printer> -o printer-error-policy=retry-job`.
- [ ] Verify the `kiosk` user can run `cupsenable`; if not, add it to `_lpadmin`.
- [ ] Test matrix on real paper: portrait and landscape × orientation option, using a grid/edge-marker calibration image to confirm symmetric overscan. Freeze the chosen options in config and in `docs/printer-notes.md`.
- [ ] Drills: remove paper mid-job, remove the ink cassette, unplug USB, power-cycle the printer, 3 back-to-back commits. Each must recover without restarting the app; missed prints are reprinted from admin.
- [ ] Soak: a Rust integration test runs 100 `prepare`+`commit` cycles with fixture covers and the Mock backend (check RSS and that journal line count == cycles). Then run 20 real prints, and leave the app running for 1 h.

Validate: options frozen and documented; all drills pass; soak shows no RSS growth over 20% and no unhandled errors.

### M8 — Packaging & event readiness
- [ ] `cargo tauri build` for the kiosk Mac's architecture, ad-hoc codesign, install to `/Applications`.
- [ ] `kiosk` user with auto-login.
- [ ] LaunchAgent with `RunAtLoad=true` and `KeepAlive = { SuccessfulExit = false }`.
- [ ] Power and notification settings: `pmset` with sleep disabled, or `caffeinate -dimsu`; Do Not Disturb on; Software Update off; screensaver off.
- [ ] One-page staff runbook: refill paper/ink, admin chord and PIN, relogin, reprint, exit.

Validate:
- A cold boot reaches Screen 1 fullscreen with no interaction within 90 s.
- After `kill -9` or a force quit, the app relaunches within 5 s, fullscreen and in a consistent state.
- After an admin exit, the app stays closed.
- Someone other than the developer completes the runbook.

---

## 7. Failure Modes & Mitigations

| Failure | Mitigation | Guest sees |
|---|---|---|
| Access token expired | Proactive refresh plus one 401 retry | Nothing |
| Rotated refresh token lost | Persisted before use; atomic store write | Manual entry until relogin |
| Refresh revoked, Premium lapsed, or account removed from allowlist | `NeedsLogin`, staff dot amber, outbox paused | Manual entry form |
| Rate limit or quota | Honour `Retry-After`; on quota, degrade and pause the outbox 15 min | Results or manual entry |
| Wi-Fi drop | Journal and USB print unaffected; cached covers or placeholder; 4G hotspot as backup | Manual entry form |
| No high-res cover | iTunes 3000 → Spotify 640 → placeholder | Slightly softer print |
| Wrong cover | Strict match, otherwise Spotify (always the correct album) | Correct cover |
| Printer paused or in error | `retry-job` policy, staff dot amber, admin "Resume printer" (`cupsenable`) | "On its way" |
| Paper or ink out | Refill reminders at 16 / 34 prints | "On its way" |
| Burst of guests | CUPS queues the jobs; Done screen shows prints waiting | "…after 2 other prints" |
| Double commit | Reducer state plus idempotency on `id` | One print |
| Crash | launchd relaunch; journal replay; unprinted list in admin (never auto-reprinted) | Back to Name within 5 s |
| Fullscreen lost or display change | Guards re-apply bounds and presentation options; native fullscreen not used | Nothing |
| Browser hidden during login | Maintenance mode | — (staff only) |

---

## 8. Pre-Event Checklist

**T-14d**
- [ ] App owner has Premium; organizer allowlisted; redirect URI registered exactly.
- [ ] Playlist created by the organizer, configured, owner check passes.
- [ ] Consumables: `ceil(guests / 36) + 1` KP-108IN-type sets.
- [ ] The organizer has signed off a sample portrait and landscape print.

**T-7d (code freeze)**
- [ ] All milestone validations pass; `npm run verify` is green on the release tag.
- [ ] Re-check the Spotify changelog.
- [ ] Soak and the 20 real prints passed.
- [ ] Release build installed under `kiosk`.

**T-1d (event hardware)**
- [ ] Fresh login in maintenance mode; one track added and then removed.
- [ ] Offline drill: Wi-Fi off → manual entry → print → Wi-Fi on → outbox drains.
- [ ] Printer defaults, `retry-job` policy and test print confirmed; tray full; spares on site; refill counters reset.
- [ ] Fullscreen checks from M1 pass on the event display.
- [ ] Sleep, updates and notifications off; power connected; journal archived and then cleared.
- [ ] Runbook printed; PIN shared with 2 staff members.

**T-2h:** cold boot → fullscreen Screen 1 → full guest cycle → print in hand → staff dot not showing.

**During:** check the paper every ~15 prints; export CSV every 2 h. **After:** final CSV export, outbox empty, unprinted list reviewed, archive `prints/` and the journal.

---

## 9. Out of Scope (cut by council)

- Windows/Linux, on-screen keyboard, separate admin window, split capabilities.
- MusicBrainz/Cover Art Archive. This is a contingency only if the M4 reference test fails, and the user must approve it first.
- iTunes *search* fallback. Manual entry is the single degraded path.
- App-level print queue and per-job completion tracking; CUPS queue plus health polling is enough.
- Universal binary, PNG DPI metadata, disk-space monitor, PIN hashing, explicit/duplicate toggles, `tauri-specta`, JS Tauri plugins.
