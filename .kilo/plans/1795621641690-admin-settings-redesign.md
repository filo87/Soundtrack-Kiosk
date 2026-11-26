# Admin / Settings redesign plan

Scope: the staff admin overlay (`src/admin/AdminOverlay.tsx`, `PinPad.tsx`). It is the only "settings page". Guest screens are out of scope. All behavior and IPC contracts stay; this is a UX/IA/visual rebuild plus two small DTO additions.

## Problems with the current page

- One 400-line scroll: status tiles, 11 equal-weight buttons, settings form, history, danger actions all at the same level. Destructive actions (Exit kiosk, Clear history) sit next to routine ones.
- Settings have no dirty state, no inline validation, no unsaved-changes guard. Save errors land in a generic message line far from the field.
- Counters ("Paper fed 12 / 16") are text; no sense of how close a refill is.
- `maintenance` is local React state and desyncs from the backend flag on reopen.
- Admin PIN is a plaintext input with no confirm/show-hide; setup values (client id, playlist URL, redirect URI) have no help text. Redirect URI is not shown anywhere in the UI.
- Global `busy` disables everything; no per-action spinner or success/failure toast.
- Backend session expires after 5 min idle but UI stays open and stale until the next command fails.

## Design

Palette rules (AGENTS.md): Wheat page, Desert Sand surfaces/inputs, Cool Steel borders/wells, Sweet Salmon for attention + primary action, Cerulean for active/selected/focus, ink `#223843`/muted `#40535E` for text. No red/amber. Do not put small text on Cerulean (white contrast 4.3); use ink on `cerulean/15` with a Cerulean indicator bar.

Layout (1280x800 kiosk, dialog ~1120 wide, max 90vh, internal scroll only in the content pane):

```
+----------------------------------------------------------+
| Soundtrack Kiosk · Admin        status chips      [Close]|
+---------+------------------------------------------------+
| Overview|  section title + one-line description          |
| Spotify |  cards ...                                     |
| Printer |                                                |
| Event   |                                                |
| Security|                                                |
| History |                                                |
+---------+------------------------------------------------+
| (sticky, only when dirty)  Unsaved changes [Discard][Save]|
+----------------------------------------------------------+
```

Sections (left nav with lucide icons, attention dot on a section when its health is off):

1. **Overview** - "What needs attention" list built from `health` (auth, network, printer state/reason, outbox pending, prints waiting, refill due, default PIN, failed prints) with a jump link to the owning section; all-clear state when empty. Consumables card: paper and ink progress bars (`count / limit`, Salmon when >= 80%, label "refill due" at limit) with **Paper refilled**, **Ink replaced**, **Reset both**. Quick actions: Test print.
2. **Spotify** - connection card (connected / login needed, Login-Relogin / Log out), client id (hint: where to find it), playlist URL with parsed playlist id preview and inline error, market and iTunes country (2-letter), read-only Redirect URI with Copy button and the "must match exactly" note.
3. **Printer** - backend badge (mock/cups), printer select (+ "System default (X)"), default orientation as a segmented control with portrait/landscape glyphs, Test print, printer state + reason.
4. **Event** - baby name, welcome line with `{babyName}` token chip and live preview of the Name screen heading.
5. **Security & kiosk** - change PIN (new + confirm, show/hide toggle, rules checklist: 4-12 digits, not 0000), Maintenance as a switch bound to backend state, **Danger zone** card: Exit kiosk.
6. **History** - filterable list (search by guest/title/artist) of last 30 with status chips (printed / failed / pending, layout, duplicate-skipped), failure reason on hover/expand, Reprint per row, Export CSV, and the **Clear local history** danger card.

Interaction rules:
- One form model for Spotify/Printer/Event/Security fields; dirty = differs from last saved baseline (counters excluded: server-owned). Sticky save bar appears only when dirty; Save disabled while invalid; Discard resets to baseline; Cmd/Ctrl+S saves. Closing with dirty state shows an in-dialog "Discard changes?" confirm (not for idle/ADMIN_LOCKED).
- Inline validation per field on blur and on save; server error from `set_config` is mapped to the field when possible (PIN, playlist) otherwise shown in a toast.
- Per-action pending state (spinner in the clicked button, others stay enabled unless they conflict); toasts (aria-live polite) replace the single message line; auto-dismiss 4s, errors persist until dismissed.
- Destructive confirms are in-dialog modals with consequences listed. Clear history requires typing `CLEAR`; Exit kiosk is a single confirm.
- Client-side idle timer (`useIdle`, 5 min) closes the overlay, mirroring the backend session.
- Focus: first nav item focused on open, arrow keys move between nav items, Esc closes (existing App handler), dialog gets `aria-labelledby`, sections `aria-labelledby`, focus returns sensibly after confirm modals.
- Motion: 150ms fade/translate on section change and modal; honor `prefers-reduced-motion`.

## Code structure

- `src/admin/AdminOverlay.tsx` - shell only: layout, nav, save bar, toasts, idle, close guard.
- `src/admin/useAdmin.ts` - state/effects: status polling on open and after actions, baseline/draft config, dirty/validity, `run(action, {pending, success})`, `fail` (ADMIN_LOCKED closes). Preserve the existing counter-merge behavior so stale drafts never overwrite counters (backend also preserves them).
- `src/admin/validation.ts` - pure validators: PIN (4-12 digits, not `0000`), playlist URL/id (mirror `parse_playlist_id`: `open.spotify.com/playlist/{id}`, `spotify:playlist:{id}`, bare id >= 10 alnum), client id (32 hex), market/country (`^[A-Za-z]{2}$`), welcome line max length.
- `src/admin/ui.tsx` - small primitives: `Card`, `Field` (label, hint, error), `TextInput`, `Select`, `Segmented`, `Switch`, `Button` (primary salmon / secondary outline / danger ink), `StatusChip`, `ProgressBar`, `Toast`, `ConfirmDialog`.
- `src/admin/sections/{Overview,Spotify,Printer,Event,Security,History}.tsx`.
- `PinPad.tsx` - same visual language: larger centered dialog, 4-12 digit input with dots, shake + lockout message from the backend, Enter submits, Esc cancels.
- Use `lucide-react` (already a dependency) and `framer-motion` (already used). No new dependencies.

## Backend / DTO changes (small)

- `AdminStatus` gains `maintenance: bool` (from `state.is_maintenance()`) and `spotifyRedirectUri: String` (from `spotify::auth::callback_redirect_uri()`). Update `commands.rs::admin_status`, `src/lib/types.ts`, and any test fixture. No new commands, so ACL files are untouched.
- No change to `set_config` rules; PIN/playlist validation messages already come back as `INVALID_INPUT`.

## Tests

- `src/admin/validation.test.ts` - table tests for every validator (accept/reject cases incl. `0000`, 3/13 digits, URL variants).
- `src/admin/AdminOverlay.test.tsx` with `mockIPC` (`admin_status`, `list_printers`, `set_config`, `mark_refilled`, `reset_printer_counters`, `reprint`, `clear_history`, `set_maintenance`):
  - loads and renders Overview; attention list reflects health; jump link switches section.
  - editing a field shows the save bar; invalid PIN disables Save; Save sends the expected `config` payload and clears dirty; Discard restores baseline.
  - closing with dirty state asks to discard.
  - `ADMIN_LOCKED` from any command calls `onClose`.
  - Clear history needs the typed `CLEAR` confirmation and shows the archive result toast.
  - maintenance switch reflects `status.maintenance` and calls `set_maintenance`.
  - idle timeout (fake timers) closes the overlay.
- Rust: update `admin_status` construction compile-time only; add no new runtime tests.
- Gate: `pnpm verify`; rebuild debug bundle.

## Docs

- `docs/runbook.md`: new section names/locations (Overview, Spotify, Printer, Event, Security, History), unsaved-changes bar, danger zone.
- `README.md` Admin section: updated action list; mention Redirect URI copy helper.
- `AGENTS.md`: add recipe note that admin UI lives in `src/admin/sections/*` and must stay within the palette.

## Order of work

1. DTO additions (Rust + TS) and fixtures.
2. `validation.ts` + tests.
3. `ui.tsx` primitives.
4. `useAdmin.ts` hook.
5. Shell + sections (Overview first, then Spotify, Printer, Event, Security, History).
6. PinPad refresh.
7. `AdminOverlay.test.tsx`.
8. Docs, `pnpm verify`, bundle rebuild, manual pass on a 1280x800 window (keyboard-only walkthrough, reduced-motion check).

## Out of scope / decisions made

- No new settings fields, no theme switching, no remote admin.
- History stays at the last 30 entries (backend limit); no pagination.
- Redirect URI remains a code constant (port 43821); the UI only displays it.
- No client-side countdown timer UI; the idle close is silent aside from the existing 5-minute hint in the header.
