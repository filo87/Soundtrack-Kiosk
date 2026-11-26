# Staff runbook — one page

## Start of event

1. Power on the Mac and the SELPHY CP1500. Wait for the printer's green light.
2. The kiosk launches automatically and fills the screen.
3. Confirm the staff dot (top-right) is not showing.
4. Load a full paper tray (18 sheets) and check the ink cassette.

## Staff access

- Press `Ctrl + Option + Shift + K`.
- On a fresh install the PIN is `0000`. Change it in **Security** before the event starts; the
  kiosk shows a salmon staff dot and refuses to save settings while the default PIN is in use.
- Enter the PIN (shared with two staff members).
- The panel opens on **Overview**. The other sections are **Spotify**, **Printer**, **Event**,
  **Security**, and **History**.
- The admin panel closes itself after 5 minutes idle, or press Esc. Closing with unsaved edits
  asks for confirmation.
- Edited settings show an **Unsaved changes** bar. **Save** commits the form; **Discard** restores
  the last saved values. Paper and ink counters update on their own.

## Refills

- Open **Overview → Consumables** to see paper and ink progress.
- **Paper:** replace the tray, then press **Paper refilled**.
- **Ink:** replace the cassette, then press **Ink replaced**.
- **Reset both** zeroes both counters at once (use before a new event or after replacing both).
- A bar turns salmon at 80% and shows **refill due** at the limit (16 paper prints, 34 ink prints).
- Check paper every ~15 prints during the event.

## Spotify

- Open **Spotify** and press **Login / relogin**; the browser opens and the kiosk temporarily
  leaves kiosk mode.
- Finish the Spotify login in the browser; the kiosk restores itself.
- The **Redirect URI** card shows the exact value for the Spotify app with a **Copy** button.
- When auth is lost the dot turns salmon and guests can still type songs manually; those entries
  are journaled and added to the playlist when login returns.

## Print options

- Guests choose **portrait** or **landscape**, each in **Cover** (full-bleed crop) or **Fit**
  (full album art with a white margin): four options in total.
- Fit is the writing-friendly option: guests stick the print into the album and write a memory or
  wish in the white margin. Cover leaves no writing space.
- ←/→ changes orientation, ↑/↓ changes cover/fit; Enter prints.

## Duplicate songs

- A song already added to the playlist is not added again, even if a different version/URI is
  picked. The second guest still gets a print.
- **History** marks these as **duplicate skipped** with the stored reason.

## Reprints

- **History** shows the last 30 selections with a **Reprint** button per row.
- Failed prints are also listed in the unprinted count; open the row to see the failure reason.

## Export and close

- **History → Export CSV** writes `~/Desktop/soundtrack-YYYYMMDD-HHMMSS.csv`.
- Export every ~2 hours and once at the end.
- **Security → Danger zone → Exit kiosk** stops the app (launchd keeps it closed after a clean
  exit).
- To leave the event, archive the journal and `prints/`, then export the final CSV.
- **History → Clear local history** requires typing `CLEAR`. It archives and empties the local
  journal and admin list. It does not change the Spotify playlist; pending playlist adds are
  archived and will not be added, and songs already in the playlist can be added again afterwards.
  Print files are kept.

## If something is wrong

| Dot | Meaning | Action |
|---|---|---|
| hidden | healthy | none |
| salmon | auth lost | Overview → Spotify → Login |
| salmon | printer stopped | check paper/ink/USB, Printer → Test print |
| salmon | prints pending > 5 min | check network, watch Overview and History |
| salmon | refill due | Overview → Consumables |
| not visible | app crashed | launchd relaunches in ~5 s; wait for screen 1 |
