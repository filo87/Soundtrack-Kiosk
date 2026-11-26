import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AdminStatus,
  ClearHistoryResult,
  CommitResult,
  Config,
  Health,
  PrepareResult,
  PrinterList,
  PrintOutcome,
  PublicConfig,
  RecentSelection,
  SearchResponse,
  SelectionInput,
  Track,
  Orientation,
  PrintLayout,
} from "./types";

export type KioskErrorCode =
  | "AUTH_REQUIRED"
  | "OFFLINE"
  | "QUOTA"
  | "RATE_LIMITED"
  | "NOT_FOUND"
  | "PRINT_FAILED"
  | "ADMIN_LOCKED"
  | "INVALID_INPUT"
  | "INTERNAL";

export class KioskError extends Error {
  readonly code: KioskErrorCode;

  constructor(code: KioskErrorCode, message: string) {
    super(message);
    this.name = "KioskError";
    this.code = code;
  }
}

const CODES: KioskErrorCode[] = [
  "AUTH_REQUIRED",
  "OFFLINE",
  "QUOTA",
  "RATE_LIMITED",
  "NOT_FOUND",
  "PRINT_FAILED",
  "ADMIN_LOCKED",
  "INVALID_INPUT",
  "INTERNAL",
];

export function toKioskError(value: unknown): KioskError {
  if (value instanceof KioskError) {
    return value;
  }
  if (typeof value === "object" && value !== null) {
    const record = value as Record<string, unknown>;
    const code = typeof record.code === "string" ? record.code : "INTERNAL";
    const message =
      typeof record.message === "string" ? record.message : "Something went wrong.";
    return new KioskError(
      (CODES as string[]).includes(code) ? (code as KioskErrorCode) : "INTERNAL",
      message,
    );
  }
  if (typeof value === "string") {
    return new KioskError("INTERNAL", value);
  }
  return new KioskError("INTERNAL", "Something went wrong.");
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw toKioskError(error);
  }
}

export const ipc = {
  ping: () => call<string>("ping"),
  searchTracks: (query: string) => call<SearchResponse>("search_tracks", { query }),
  prefetchCover: (track: Track) => call<void>("prefetch_cover", { track }),
  prepareSelection: (input: SelectionInput) =>
    call<PrepareResult>("prepare_selection", { input }),
  discardPrepared: (prepId: string) => call<void>("discard_preparation", { prepId }),
  commitSelection: (args: {
    id: string;
    prepId: string;
    guestName: string;
    orientation: Orientation;
    layout: PrintLayout;
  }) => call<CommitResult>("commit_selection", args),

  adminUnlock: (pin: string) => call<void>("admin_unlock", { pin }),
  adminLock: () => call<void>("admin_lock"),
  adminStatus: () => call<AdminStatus>("admin_status"),
  authStart: () => call<void>("auth_start"),
  authLogout: () => call<void>("auth_logout"),
  getPublicConfig: () => call<PublicConfig>("get_public_config"),
  getConfig: () => call<Config>("get_config"),
  setConfig: (config: Config) => call<Config>("set_config", { config }),
  listPrinters: () => call<PrinterList>("list_printers"),
  testPrint: () => call<PrintOutcome>("test_print"),
  listRecent: (limit = 30) => call<RecentSelection[]>("list_recent", { limit }),
  reprint: (id: string) => call<CommitResult>("reprint", { id }),
  exportCsv: () => call<string>("export_csv"),
  clearHistory: () => call<ClearHistoryResult>("clear_history"),
  markRefilled: (kind: "paper" | "ink") => call<Config>("mark_refilled", { kind }),
  resetPrinterCounters: () => call<Config>("reset_printer_counters"),
  setMaintenance: (enabled: boolean) => call<void>("set_maintenance", { enabled }),
  exitKiosk: () => call<void>("exit_kiosk"),
};

export function onHealth(handler: (health: Health) => void): Promise<UnlistenFn> {
  return listen<Health>("kiosk://health", (event) => handler(event.payload));
}
