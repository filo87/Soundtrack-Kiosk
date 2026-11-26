import { useCallback, useEffect, useId, useRef, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent } from "react";
import { motion, useReducedMotion } from "framer-motion";
import {
  Gift,
  History as HistoryIcon,
  LayoutDashboard,
  Music,
  Printer as PrinterIcon,
  Shield,
  X,
  type LucideIcon,
} from "lucide-react";
import { useIdle } from "../lib/useIdle";
import { useAdmin } from "./useAdmin";
import { Button, ConfirmDialog, StatusChip, ToastStack } from "./ui";
import { EventSection } from "./sections/Event";
import { HistorySection } from "./sections/History";
import { Overview } from "./sections/Overview";
import { PrinterSection } from "./sections/Printer";
import { SecuritySection } from "./sections/Security";
import { SpotifySection } from "./sections/Spotify";
import { isPrinterOk, printerTone, SECTION_LABELS, type SectionId } from "./sections/shared";

interface SectionMeta {
  id: SectionId;
  title: string;
  description: string;
  icon: LucideIcon;
}

const SECTION_META: Record<SectionId, SectionMeta> = {
  overview: {
    id: "overview",
    title: "Overview",
    description: "Health, consumables and the actions staff reach for first.",
    icon: LayoutDashboard,
  },
  spotify: {
    id: "spotify",
    title: "Spotify",
    description: "Connection, playlist target and the redirect URI.",
    icon: Music,
  },
  printer: {
    id: "printer",
    title: "Printer",
    description: "Queue, default orientation and a test print.",
    icon: PrinterIcon,
  },
  event: {
    id: "event",
    title: "Event",
    description: "Baby name and the welcome line guests see first.",
    icon: Gift,
  },
  security: {
    id: "security",
    title: "Security",
    description: "Admin PIN, maintenance mode and kiosk exit.",
    icon: Shield,
  },
  history: {
    id: "history",
    title: "History",
    description: "Recent prints, reprints and local history.",
    icon: HistoryIcon,
  },
};

const NAV = Object.values(SECTION_META);
const IDLE_MS = 5 * 60 * 1000;

function sectionAttention(id: SectionId, admin: ReturnType<typeof useAdmin>): boolean {
  const status = admin.status;
  if (!status) return false;
  const { health } = status;
  switch (id) {
    case "overview":
      return health.attention;
    case "spotify":
      return !health.auth || health.outboxPending > 0;
    case "printer":
      return !isPrinterOk(health.printer.state) || health.printsWaiting > 0;
    case "event":
      return false;
    case "security":
      return status.config.pin === "0000";
    case "history":
      return status.unprinted.some((selection) => selection.printState === "failed");
  }
}

export default function AdminOverlay({ onClose }: { onClose: () => void }) {
  const admin = useAdmin({ onClose });
  const { dirty, valid, saving, save, discard, toasts, dismissToast } = admin;
  const [active, setActive] = useState<SectionId>("overview");
  const [discardOpen, setDiscardOpen] = useState(false);
  const reduced = useReducedMotion();
  const titleId = useId();
  const sectionTitleId = `${titleId}-section`;
  const navRefs = useRef<Array<HTMLButtonElement | null>>([]);

  const requestClose = useCallback(() => {
    if (dirty) setDiscardOpen(true);
    else onClose();
  }, [dirty, onClose]);
  const closeRef = useRef(requestClose);
  closeRef.current = requestClose;

  useIdle(IDLE_MS, onClose, "admin");

  useEffect(() => {
    navRefs.current[0]?.focus();
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (document.querySelector('[role="alertdialog"]')) return;
      event.preventDefault();
      closeRef.current();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== "s") return;
      event.preventDefault();
      if (dirty && valid && !saving) void save();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dirty, valid, saving, save]);

  const onNavKey = (event: ReactKeyboardEvent<HTMLButtonElement>, index: number) => {
    let next: number | null = null;
    if (event.key === "ArrowDown") next = (index + 1) % NAV.length;
    else if (event.key === "ArrowUp") next = (index - 1 + NAV.length) % NAV.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = NAV.length - 1;
    if (next === null) return;
    event.preventDefault();
    const target = NAV[next];
    if (!target) return;
    setActive(target.id);
    navRefs.current[next]?.focus();
  };

  const meta = SECTION_META[active];
  const health = admin.status?.health;

  return (
    <div className="absolute inset-0 z-20 flex items-center justify-center bg-ink/55 p-6">
      <motion.div
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        initial={{ opacity: 0, scale: reduced ? 1 : 0.99 }}
        animate={{ opacity: 1, scale: 1 }}
        transition={{ duration: reduced ? 0 : 0.15 }}
        className="relative flex h-[min(90vh,800px)] w-[min(1120px,94vw)] flex-col overflow-hidden rounded-3xl border border-steel/60 bg-wheat shadow-2xl"
      >
        <header className="flex flex-wrap items-center gap-4 border-b border-steel/50 px-6 py-4">
          <div className="min-w-0">
            <h2 id={titleId} className="text-lg font-medium text-ink">
              Soundtrack Kiosk · Admin
            </h2>
            <p className="text-xs text-muted">Session ends after 5 minutes idle.</p>
          </div>
          <div className="ml-auto flex flex-wrap items-center gap-2">
            {health ? (
              <>
                <StatusChip tone={health.auth ? "ok" : "attention"}>
                  Spotify {health.auth ? "connected" : "login"}
                </StatusChip>
                <StatusChip tone={health.network ? "ok" : "attention"}>
                  Network {health.network ? "online" : "offline"}
                </StatusChip>
                <StatusChip tone={printerTone(health.printer.state)}>
                  Printer {health.printer.state}
                </StatusChip>
                {admin.status?.maintenance ? (
                  <StatusChip tone="attention">Maintenance</StatusChip>
                ) : null}
              </>
            ) : (
              <StatusChip tone="neutral">Loading</StatusChip>
            )}
            <Button variant="ghost" onClick={requestClose}>
              <X aria-hidden className="h-4 w-4" />
              Close
            </Button>
          </div>
        </header>

        <div className="flex min-h-0 flex-1">
          <nav
            aria-label="Admin sections"
            className="w-56 shrink-0 overflow-y-auto border-r border-steel/50 bg-wheat/50 p-3"
          >
            <ul className="space-y-1">
              {NAV.map((section, index) => {
                const Icon = section.icon;
                const selected = section.id === active;
                return (
                  <li key={section.id}>
                    <button
                      ref={(element) => {
                        navRefs.current[index] = element;
                      }}
                      type="button"
                      aria-current={selected ? "page" : undefined}
                      onClick={() => setActive(section.id)}
                      onKeyDown={(event) => onNavKey(event, index)}
                      className={`flex w-full items-center gap-2.5 rounded-xl px-3 py-2.5 text-left text-sm font-medium transition ${
                        selected
                          ? "bg-cerulean/15 text-ink shadow-[inset_3px_0_0_0_var(--color-cerulean)]"
                          : "text-muted hover:bg-cerulean/10 hover:text-ink"
                      }`}
                    >
                      <Icon aria-hidden className="h-4 w-4 shrink-0" />
                      <span>{SECTION_LABELS[section.id]}</span>
                      {sectionAttention(section.id, admin) ? (
                        <span
                          role="img"
                          title={`${SECTION_LABELS[section.id]} needs attention`}
                          aria-label={`${SECTION_LABELS[section.id]} needs attention`}
                          className="ml-auto h-2 w-2 shrink-0 rounded-full bg-salmon"
                        />
                      ) : null}
                    </button>
                  </li>
                );
              })}
            </ul>
          </nav>

          <main className="min-w-0 flex-1 overflow-y-auto p-6">
            {admin.loading && !admin.config ? (
              <p className="text-sm text-muted">Loading admin…</p>
            ) : (
              <>
                <div className="mb-5">
                  <h2 id={sectionTitleId} className="text-2xl font-medium text-ink">
                    {meta.title}
                  </h2>
                  <p className="mt-1 text-sm text-muted">{meta.description}</p>
                </div>
                <motion.section
                  key={active}
                  aria-labelledby={sectionTitleId}
                  initial={{ opacity: 0, y: reduced ? 0 : 6 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ duration: reduced ? 0 : 0.15 }}
                >
                  {active === "overview" ? <Overview admin={admin} onNavigate={setActive} /> : null}
                  {active === "spotify" ? <SpotifySection admin={admin} /> : null}
                  {active === "printer" ? <PrinterSection admin={admin} /> : null}
                  {active === "event" ? <EventSection admin={admin} /> : null}
                  {active === "security" ? <SecuritySection admin={admin} /> : null}
                  {active === "history" ? <HistorySection admin={admin} /> : null}
                </motion.section>
              </>
            )}
          </main>
        </div>

        {dirty ? (
          <footer className="flex flex-wrap items-center gap-3 border-t border-steel/60 bg-wheat px-6 py-3">
            <div>
              <p className="text-sm font-medium text-ink">Unsaved changes</p>
              <p className="text-xs text-muted">Paper and ink counters update automatically.</p>
            </div>
            <div className="ml-auto flex items-center gap-2">
              <Button onClick={discard} disabled={saving}>
                Discard
              </Button>
              <Button
                variant="primary"
                onClick={() => void save()}
                pending={saving}
                disabled={!valid}
              >
                Save
              </Button>
            </div>
          </footer>
        ) : null}

        <ToastStack toasts={toasts} onDismiss={dismissToast} />

        {discardOpen ? (
          <ConfirmDialog
            title="Discard changes?"
            confirmLabel="Discard changes"
            cancelLabel="Keep editing"
            danger
            onCancel={() => setDiscardOpen(false)}
            onConfirm={() => {
              admin.discard();
              setDiscardOpen(false);
              onClose();
            }}
          >
            Your edits have not been saved. Closing now keeps the last saved settings.
          </ConfirmDialog>
        ) : null}
      </motion.div>
    </div>
  );
}
