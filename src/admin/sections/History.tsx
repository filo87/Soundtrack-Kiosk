import { useState } from "react";
import { Download, RotateCcw, Search, Trash2 } from "lucide-react";
import { ipc } from "../../lib/ipc";
import type { AdminController } from "../useAdmin";
import { Button, Card, ConfirmDialog, StatusChip, TextInput } from "../ui";
import { printChip, selectionMatches } from "./shared";

export function HistorySection({ admin }: { admin: AdminController }) {
  const { status } = admin;
  const [query, setQuery] = useState("");
  const [confirmClear, setConfirmClear] = useState(false);
  if (!status) return null;

  const filtered = status.recent.filter((selection) => selectionMatches(selection, query));

  return (
    <div className="space-y-5">
      <Card
        title="Recent selections"
        description="The last 30 journal entries, newest first."
        actions={
          <Button
            onClick={() =>
              void admin.run("export-csv", () => ipc.exportCsv(), {
                success: (path) => `CSV exported to ${path}`,
              })
            }
            pending={admin.isPending("export-csv")}
          >
            <Download aria-hidden className="h-4 w-4" />
            Export CSV
          </Button>
        }
      >
        <label className="relative mb-4 block max-w-sm">
          <span className="sr-only">Search history</span>
          <Search
            aria-hidden
            className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted"
          />
          <TextInput
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            aria-label="Search history"
            placeholder="Guest, title or artist"
            autoComplete="off"
            className="pl-9"
          />
        </label>

        {filtered.length ? (
          <ul className="space-y-2">
            {filtered.map((selection) => {
              const chip = printChip(selection);
              return (
                <li
                  key={selection.id}
                  className="flex flex-wrap items-start justify-between gap-3 rounded-xl border border-steel/50 bg-wheat/60 p-3.5"
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <p className="min-w-0 truncate font-medium text-ink">
                        {selection.guestName} — {selection.trackTitle}
                      </p>
                      <StatusChip tone={chip.tone}>{chip.label}</StatusChip>
                      <StatusChip tone="neutral">
                        {selection.orientation} / {selection.layout}
                      </StatusChip>
                      {selection.playlistSkipped ? (
                        <StatusChip tone="neutral">duplicate skipped</StatusChip>
                      ) : null}
                    </div>
                    <p className="mt-1 truncate text-sm text-muted">
                      {selection.artist} · {selection.timestamp}
                    </p>
                    {selection.printReason ? (
                      <p className="mt-1.5 rounded-lg bg-salmon/20 px-2.5 py-1 text-xs text-ink">
                        {selection.printReason}
                      </p>
                    ) : null}
                    {selection.playlistSkipReason ? (
                      <p className="mt-1 text-xs text-muted">
                        Playlist: {selection.playlistSkipReason}
                      </p>
                    ) : null}
                  </div>
                  <Button
                    size="sm"
                    className="shrink-0"
                    onClick={() =>
                      void admin.run(`reprint-${selection.id}`, () => ipc.reprint(selection.id), {
                        onSuccess: (result) => {
                          admin.pushToast(
                            result.printQueued ? "success" : "error",
                            result.printQueued
                              ? "Reprint sent."
                              : "Reprint failed. Check the printer and try again.",
                          );
                        },
                      })
                    }
                    pending={admin.isPending(`reprint-${selection.id}`)}
                  >
                    <RotateCcw aria-hidden className="h-3.5 w-3.5" />
                    Reprint
                  </Button>
                </li>
              );
            })}
          </ul>
        ) : (
          <p className="rounded-xl bg-wheat/60 px-4 py-6 text-center text-sm text-muted">
            {status.recent.length ? "No selections match that search." : "No selections yet."}
          </p>
        )}

        {status.unprinted.length ? (
          <p className="mt-3 text-xs text-muted">
            {status.unprinted.length} unprinted selection
            {status.unprinted.length === 1 ? "" : "s"} (pending or failed). Failed rows can be
            reprinted.
          </p>
        ) : null}
      </Card>

      <Card
        title="Clear local history"
        description="Archive the local journal and start with an empty list."
        className="border-salmon/70 bg-salmon/10"
      >
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div className="max-w-2xl">
            <p className="text-sm text-muted">
              Archives and empties the local journal, drops pending playlist adds, and resets local
              duplicate memory so songs already in the playlist can be added again. The Spotify
              playlist and print files are not changed.
            </p>
          </div>
          <Button variant="danger" onClick={() => setConfirmClear(true)}>
            <Trash2 aria-hidden className="h-4 w-4" />
            Clear local history
          </Button>
        </div>
      </Card>

      {confirmClear ? (
        <ConfirmDialog
          title="Clear local history?"
          confirmLabel="Clear history"
          danger
          requireText="CLEAR"
          pending={admin.isPending("clear-history")}
          onCancel={() => setConfirmClear(false)}
          onConfirm={() =>
            void admin.run("clear-history", () => ipc.clearHistory(), {
              success: (result) => {
                const pending =
                  result.pendingArchived > 0
                    ? ` ${result.pendingArchived} pending playlist add${
                        result.pendingArchived === 1 ? "" : "s"
                      } were archived and will not be added.`
                    : "";
                return `Cleared ${result.archived} local selection${
                  result.archived === 1 ? "" : "s"
                }.${pending}${result.archivePath ? ` Archive: ${result.archivePath}` : ""}`;
              },
              onSuccess: () => setConfirmClear(false),
            })
          }
        >
          The journal is archived to a timestamped file and the admin list is emptied. Pending
          playlist adds are archived, not sent. Songs already in the playlist can be added again
          afterwards. Print files are kept.
        </ConfirmDialog>
      ) : null}
    </div>
  );
}
