import type { RecentSelection } from "../../lib/types";
import type { ChipTone } from "../ui";

export type SectionId = "overview" | "spotify" | "printer" | "event" | "security" | "history";

export const SECTION_LABELS: Record<SectionId, string> = {
  overview: "Overview",
  spotify: "Spotify",
  printer: "Printer",
  event: "Event",
  security: "Security",
  history: "History",
};

export function isPrinterOk(state: string): boolean {
  return ["idle", "printing", "processing"].includes(state);
}

export function printerTone(state: string): ChipTone {
  return isPrinterOk(state) ? "ok" : "attention";
}

export function printChip(selection: RecentSelection): { label: string; tone: ChipTone } {
  switch (selection.printState) {
    case "sent":
      return { label: "printed", tone: "ok" };
    case "failed":
      return { label: "failed", tone: "attention" };
    default:
      return { label: "pending", tone: "neutral" };
  }
}

export function selectionMatches(selection: RecentSelection, query: string): boolean {
  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  return [selection.guestName, selection.trackTitle, selection.artist].some((value) =>
    value.toLowerCase().includes(needle),
  );
}
