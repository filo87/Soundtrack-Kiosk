# Printer notes — Canon SELPHY CP1500

Status: code defaults are frozen provisionally; the values below must be re-confirmed on the event
unit before the paper test matrix is signed off.

## Transport

- USB connection, CUPS queue created by macOS.
- Backend selection: a configured queue prints through CUPS even in a debug build; with no queue
  selected, debug builds use the mock backend and release builds use CUPS.
- `BABY_KIOSK_PRINTER=mock|cups` overrides the configured-queue rule.
- The CUPS queue name is stored in kiosk settings (`printer`) and selected in the admin overlay.
- If no queue is selected, the app falls back to `lpstat -d` (CUPS default destination), so the
  admin “Default” option is functional once the printer is the system default.
- The queue list in admin always comes from CUPS (`lpstat -e`), so development builds show the real
  queues. The mock backend only adds a synthetic `Mock` entry when CUPS reports no queues and no
  queue is configured.

## Options used by the app

`src-tauri/src/print.rs` submits:

```text
lp -d <queue> \
  -o media=Postcard \
  -o print-scaling=fill \
  -o orientation-requested=<3|4> \
  <file>
```

- `orientation-requested=3` is portrait, `4` is landscape.
- The app already renders the exact 1200x1800 (portrait) or 1800x1200 (landscape) PNG, so
  `print-scaling=fill` should not crop further. Keep it as a safety net.
- `media=Postcard` is the 10x15 cm stock. If the queue exposes a different media keyword,
  update `MEDIA_OPTION` in `print.rs` and this file together.

## First hardware session

```bash
lpstat -p CP1500
lpoptions -p CP1500 -l
lpoptions -p CP1500 -o media=Postcard -o print-scaling=fill
lpadmin -p CP1500 -o printer-error-policy=retry-job
```

Record below, from `lpoptions -l`:

| Option | Chosen value | Notes |
|---|---|---|
| media | `Postcard` | |
| borderless | _pending hardware_ | gutenprint may use `StpBorderless` or `borderless` |
| orientation-requested | `3` / `4` | portrait / landscape |
| print-scaling | `fill` | |

If the queue requires a borderless option that is not in the app, add it to the `lp` arguments and
freeze it here.

## Calibration matrix

Print a grid/edge-marker image in four combinations:

1. portrait file, `orientation-requested=3`
2. portrait file, `orientation-requested=4`
3. landscape file, `orientation-requested=4`
4. landscape file, `orientation-requested=3`

The correct pair keeps the image upright with symmetric overscan. Freeze only the pair that does.

## Drills

Each must recover without restarting the app:

- open the paper door mid-job
- remove the ink cassette mid-job
- unplug USB and reconnect
- power-cycle the printer
- three back-to-back commits

Reprints use the staged file in `prints/YYYY-MM-DD/` from the admin overlay.

## Permissions

Verify the `kiosk` user can run `cupsenable`; if not, add it to `_lpadmin`.

```bash
dscl . -read /Groups/_lpadmin GroupMembership
```
