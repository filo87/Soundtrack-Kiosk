import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { AdminStatus, Config, RecentSelection } from "../lib/types";
import AdminOverlay from "./AdminOverlay";

const baseConfig: Config = {
  babyName: "Ada",
  welcomeLine: "A song for {babyName}",
  playlistUrl: "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M",
  market: "US",
  country: "US",
  defaultOrientation: "portrait",
  printer: null,
  pin: "2468",
  spotifyClientId: "a1b2c3d4e5f60718293a4b5c6d7e8f90",
  paperCount: 3,
  inkCount: 4,
};

const sentSelection: RecentSelection = {
  id: "s1",
  guestName: "Ada",
  trackTitle: "Halo",
  artist: "Beyoncé",
  timestamp: "2026-11-25T10:00:00Z",
  orientation: "portrait",
  layout: "cover",
  mode: "track",
  trackUri: "spotify:track:t1",
  printState: "sent",
  printFile: "/prints/1.png",
  printReason: null,
  playlistSkipped: false,
  playlistSkipReason: null,
};

const failedSelection: RecentSelection = {
  ...sentSelection,
  id: "s2",
  guestName: "Grace",
  trackTitle: "Yellow",
  printState: "failed",
  printReason: "Out of paper",
};

function makeStatus(
  options: {
    auth?: boolean;
    attention?: boolean;
    maintenance?: boolean;
    recent?: RecentSelection[];
    unprinted?: RecentSelection[];
  } = {},
): AdminStatus {
  const auth = options.auth ?? true;
  return {
    health: {
      auth,
      network: true,
      printer: { state: "idle", reason: null },
      printsWaiting: 0,
      outboxPending: 0,
      refillDue: false,
      paperLimit: 16,
      inkLimit: 34,
      attention: options.attention ?? !auth,
    },
    config: { ...baseConfig },
    printBackend: "mock",
    maintenance: options.maintenance ?? false,
    spotifyRedirectUri: "http://127.0.0.1:43821/callback",
    recent: options.recent ?? [sentSelection],
    unprinted: options.unprinted ?? [],
  };
}

let currentStatus: AdminStatus;
let lastSetConfig: Config | null = null;
let setMaintenanceCalls: boolean[] = [];
let clearHistoryCalls = 0;
let adminStatusError: { code: string; message: string } | null = null;

function install() {
  lastSetConfig = null;
  setMaintenanceCalls = [];
  clearHistoryCalls = 0;
  adminStatusError = null;
  mockIPC((cmd, payload) => {
    switch (cmd) {
      case "admin_status":
        if (adminStatusError) throw adminStatusError;
        return currentStatus;
      case "list_printers":
        return { printers: ["SELPHY"], default: "SELPHY" };
      case "set_config": {
        const { config } = payload as { config: Config };
        lastSetConfig = config;
        currentStatus = {
          ...currentStatus,
          config: {
            ...config,
            paperCount: currentStatus.config.paperCount,
            inkCount: currentStatus.config.inkCount,
          },
        };
        return currentStatus.config;
      }
      case "mark_refilled": {
        const { kind } = payload as { kind: "paper" | "ink" };
        const config = {
          ...currentStatus.config,
          ...(kind === "paper" ? { paperCount: 0 } : { inkCount: 0 }),
        };
        currentStatus = { ...currentStatus, config };
        return config;
      }
      case "reset_printer_counters": {
        const config = { ...currentStatus.config, paperCount: 0, inkCount: 0 };
        currentStatus = { ...currentStatus, config };
        return config;
      }
      case "set_maintenance": {
        const { enabled } = payload as { enabled: boolean };
        setMaintenanceCalls.push(enabled);
        currentStatus = { ...currentStatus, maintenance: enabled };
        return null;
      }
      case "clear_history":
        clearHistoryCalls += 1;
        return {
          archived: 3,
          pendingArchived: 1,
          archivePath: "/tmp/journal-archive.jsonl",
        };
      case "export_csv":
        return "/tmp/soundtrack.csv";
      case "reprint":
        return { printsAhead: 0, printQueued: true, printFile: "/tmp/reprint.png" };
      case "test_print":
        return { job: "1", file: "/tmp/test.png" };
      case "admin_lock":
        return null;
      default:
        return null;
    }
  });
}

function renderOverlay(onClose = vi.fn()) {
  currentStatus = makeStatus();
  install();
  render(<AdminOverlay onClose={onClose} />);
  return onClose;
}

describe("AdminOverlay", () => {
  afterEach(() => {
    cleanup();
    clearMocks();
    vi.useRealTimers();
  });

  it("renders Overview and jumps to an attention section", async () => {
    currentStatus = makeStatus({ auth: false });
    install();
    const onClose = vi.fn();
    const user = userEvent.setup();
    render(<AdminOverlay onClose={onClose} />);

    expect(await screen.findByText("Spotify is not connected")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Open Spotify" }));

    expect(await screen.findByRole("heading", { level: 2, name: "Spotify" })).toBeInTheDocument();
    expect(screen.getByText("Playlist id: 37i9dQZF1DXcBWIGoYBM5M")).toBeInTheDocument();
    expect(screen.getByLabelText("Redirect URI")).toHaveValue("http://127.0.0.1:43821/callback");
  });

  it("shows the save bar, saves the expected config, then discards later edits", async () => {
    const onClose = renderOverlay();
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: "Event" }));
    const input = await screen.findByLabelText("Baby name");
    await user.clear(input);
    await user.type(input, "Grace");

    expect(await screen.findByText("Unsaved changes")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(lastSetConfig).not.toBeNull());
    expect(lastSetConfig?.babyName).toBe("Grace");
    expect(lastSetConfig?.paperCount).toBe(3);
    expect(lastSetConfig?.inkCount).toBe(4);
    await waitFor(() => expect(screen.queryByText("Unsaved changes")).not.toBeInTheDocument());

    await user.clear(input);
    await user.type(input, "Linus");
    expect(await screen.findByText("Unsaved changes")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Discard" }));
    expect(screen.getByLabelText("Baby name")).toHaveValue("Grace");
    expect(screen.queryByText("Unsaved changes")).not.toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("disables Save while the PIN is invalid", async () => {
    renderOverlay();
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: "Security" }));
    const pin = await screen.findByLabelText("New PIN");
    await user.clear(pin);
    await user.type(pin, "12");

    expect(await screen.findByText("Unsaved changes")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  });

  it("asks before closing with unsaved changes", async () => {
    const onClose = renderOverlay();
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: "Event" }));
    const input = await screen.findByLabelText("Welcome line");
    await user.type(input, "!");
    await user.click(screen.getByRole("button", { name: "Close" }));

    expect(await screen.findByRole("alertdialog")).toBeInTheDocument();
    expect(screen.getByText("Discard changes?")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Keep editing" }));
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Close" }));
    await user.click(screen.getByRole("button", { name: "Discard changes" }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("closes when a command reports ADMIN_LOCKED", async () => {
    currentStatus = makeStatus();
    install();
    adminStatusError = { code: "ADMIN_LOCKED", message: "Admin session expired." };
    const onClose = vi.fn();
    render(<AdminOverlay onClose={onClose} />);

    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it("requires typing CLEAR before clearing history and shows the archive toast", async () => {
    currentStatus = makeStatus({ recent: [sentSelection, failedSelection] });
    install();
    const user = userEvent.setup();
    render(<AdminOverlay onClose={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: "History" }));
    await user.click(await screen.findByRole("button", { name: "Clear local history" }));

    const confirm = screen.getByRole("button", { name: "Clear history" });
    expect(confirm).toBeDisabled();
    await user.type(screen.getByLabelText("Type CLEAR to confirm"), "CLEAR");
    expect(confirm).toBeEnabled();
    await user.click(confirm);

    await waitFor(() => expect(clearHistoryCalls).toBe(1));
    expect(await screen.findByText(/Cleared 3 local selections/)).toBeInTheDocument();
    expect(screen.getByText(/1 pending playlist add/)).toBeInTheDocument();
    expect(screen.getByText(/Archive: \/tmp\/journal-archive.jsonl/)).toBeInTheDocument();
  });

  it("reflects backend maintenance state and toggles it", async () => {
    currentStatus = makeStatus({ maintenance: true });
    install();
    const user = userEvent.setup();
    render(<AdminOverlay onClose={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: "Security" }));
    const toggle = await screen.findByRole("switch", { name: "Maintenance mode" });
    expect(toggle).toHaveAttribute("aria-checked", "true");

    await user.click(toggle);
    await waitFor(() => expect(setMaintenanceCalls).toEqual([false]));
    await waitFor(() =>
      expect(screen.getByRole("switch", { name: "Maintenance mode" })).toHaveAttribute(
        "aria-checked",
        "false",
      ),
    );
  });

  it("closes after five minutes idle", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    currentStatus = makeStatus();
    install();
    const onClose = vi.fn();
    render(<AdminOverlay onClose={onClose} />);

    expect(await screen.findByText("What needs attention")).toBeInTheDocument();
    act(() => {
      vi.advanceTimersByTime(5 * 60 * 1000);
    });
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
