import {
  AlertCircle,
  ArrowRight,
  CheckCircle2,
  Droplet,
  FlaskConical,
  Layers,
  RotateCcw,
} from "lucide-react";
import { ipc } from "../../lib/ipc";
import type { AdminController } from "../useAdmin";
import { Button, Card, ProgressBar, StatusChip } from "../ui";
import { isPrinterOk, SECTION_LABELS, type SectionId } from "./shared";

interface AttentionItem {
  id: string;
  title: string;
  detail: string;
  section?: SectionId;
}

export function Overview({
  admin,
  onNavigate,
}: {
  admin: AdminController;
  onNavigate: (section: SectionId) => void;
}) {
  const { status, config } = admin;
  if (!status || !config) return null;
  const { health } = status;
  const failed = status.unprinted.filter((selection) => selection.printState === "failed").length;
  const attention: AttentionItem[] = [];

  if (!health.auth) {
    attention.push({
      id: "auth",
      title: "Spotify is not connected",
      detail: "Guests can type songs manually; playlist adds resume after a staff login.",
      section: "spotify",
    });
  }
  if (!health.network) {
    attention.push({
      id: "network",
      title: "The kiosk is offline",
      detail: "Search, covers and playlist sync will retry when the network returns.",
    });
  }
  if (!isPrinterOk(health.printer.state)) {
    attention.push({
      id: "printer",
      title: "The printer needs attention",
      detail: health.printer.reason
        ? health.printer.reason
        : `Printer state is ${health.printer.state}.`,
      section: "printer",
    });
  }
  if (health.printsWaiting > 0) {
    attention.push({
      id: "prints-waiting",
      title: `${health.printsWaiting} print${health.printsWaiting === 1 ? "" : "s"} waiting`,
      detail: "The queue is backed up. Check paper, ink and the USB connection.",
      section: "printer",
    });
  }
  if (health.outboxPending > 0) {
    attention.push({
      id: "outbox",
      title: `${health.outboxPending} playlist add${health.outboxPending === 1 ? "" : "s"} pending`,
      detail: "Songs are journaled and will be added when Spotify is reachable.",
      section: "spotify",
    });
  }
  if (health.refillDue) {
    attention.push({
      id: "refill",
      title: "A refill is due",
      detail: "Reset the paper or ink counter after replacing the supply.",
    });
  }
  if (config.pin === "0000") {
    attention.push({
      id: "pin",
      title: "The default admin PIN is in use",
      detail: "Set a 4-12 digit PIN before the event opens.",
      section: "security",
    });
  }
  if (failed > 0) {
    attention.push({
      id: "failed",
      title: `${failed} print${failed === 1 ? "" : "s"} failed`,
      detail: "Open History to see the reason and reprint.",
      section: "history",
    });
  }
  if (health.attention && attention.length === 0) {
    attention.push({
      id: "attention",
      title: "Staff attention needed",
      detail: "Check the status chips and recent history.",
    });
  }

  const paperDue = config.paperCount >= health.paperLimit;
  const inkDue = config.inkCount >= health.inkLimit;

  return (
    <div className="space-y-5">
      <Card
        title="What needs attention"
        description={
          attention.length
            ? "Work through these before the next guest."
            : "Everything is ready for guests."
        }
      >
        {attention.length ? (
          <ul className="space-y-2.5">
            {attention.map((item) => (
              <li
                key={item.id}
                className="flex items-start gap-3 rounded-xl border border-steel/50 bg-wheat/60 p-3.5"
              >
                <AlertCircle aria-hidden className="mt-0.5 h-5 w-5 shrink-0 text-ink" />
                <div className="min-w-0 flex-1">
                  <p className="font-medium text-ink">{item.title}</p>
                  <p className="mt-0.5 text-sm text-muted">{item.detail}</p>
                </div>
                {item.section ? (
                  <Button
                    size="sm"
                    onClick={() => onNavigate(item.section as SectionId)}
                    className="shrink-0"
                  >
                    Open {SECTION_LABELS[item.section]}
                    <ArrowRight aria-hidden className="h-3.5 w-3.5" />
                  </Button>
                ) : null}
              </li>
            ))}
          </ul>
        ) : (
          <div className="flex items-center gap-3 rounded-xl bg-cerulean/10 p-4">
            <CheckCircle2 aria-hidden className="h-6 w-6 text-cerulean" />
            <div>
              <p className="font-medium text-ink">All clear</p>
              <p className="text-sm text-muted">
                Spotify, the printer and both consumable counters look good.
              </p>
            </div>
          </div>
        )}
      </Card>

      <div className="grid gap-5 lg:grid-cols-[1.5fr_1fr]">
        <Card
          title="Consumables"
          description="Counters advance with every print. Reset one after replacing a supply."
        >
          <div className="grid gap-5 sm:grid-cols-2">
            <div>
              <div className="mb-2 flex items-baseline justify-between gap-2">
                <span className="flex items-center gap-2 font-medium text-ink">
                  <Layers aria-hidden className="h-4 w-4 text-muted" />
                  Paper
                </span>
                <span className="text-sm text-muted">
                  {config.paperCount} / {health.paperLimit}
                </span>
              </div>
              <ProgressBar
                label="Paper prints since refill"
                value={config.paperCount}
                max={health.paperLimit}
              />
              <p className="mt-2 text-xs text-muted">
                {paperDue ? (
                  <StatusChip tone="attention">refill due</StatusChip>
                ) : (
                  `${health.paperLimit - config.paperCount} prints until refill`
                )}
              </p>
            </div>
            <div>
              <div className="mb-2 flex items-baseline justify-between gap-2">
                <span className="flex items-center gap-2 font-medium text-ink">
                  <Droplet aria-hidden className="h-4 w-4 text-muted" />
                  Ink
                </span>
                <span className="text-sm text-muted">
                  {config.inkCount} / {health.inkLimit}
                </span>
              </div>
              <ProgressBar
                label="Ink prints since replacement"
                value={config.inkCount}
                max={health.inkLimit}
              />
              <p className="mt-2 text-xs text-muted">
                {inkDue ? (
                  <StatusChip tone="attention">refill due</StatusChip>
                ) : (
                  `${health.inkLimit - config.inkCount} prints until refill`
                )}
              </p>
            </div>
          </div>
          <div className="mt-5 flex flex-wrap gap-2">
            <Button
              onClick={() =>
                void admin.run("refill-paper", () => ipc.markRefilled("paper"), {
                  success: "Paper counter reset.",
                })
              }
              pending={admin.isPending("refill-paper")}
            >
              <Layers aria-hidden className="h-4 w-4" />
              Paper refilled
            </Button>
            <Button
              onClick={() =>
                void admin.run("refill-ink", () => ipc.markRefilled("ink"), {
                  success: "Ink counter reset.",
                })
              }
              pending={admin.isPending("refill-ink")}
            >
              <Droplet aria-hidden className="h-4 w-4" />
              Ink replaced
            </Button>
            <Button
              variant="ghost"
              onClick={() =>
                void admin.run("reset-counters", () => ipc.resetPrinterCounters(), {
                  success: "Printer counters reset.",
                })
              }
              pending={admin.isPending("reset-counters")}
            >
              <RotateCcw aria-hidden className="h-4 w-4" />
              Reset both
            </Button>
          </div>
        </Card>

        <Card title="Quick actions" description="Useful before guests arrive.">
          <Button
            variant="primary"
            className="w-full"
            onClick={() =>
              void admin.run("test-print", () => ipc.testPrint(), {
                success: "Test print sent.",
              })
            }
            pending={admin.isPending("test-print")}
          >
            <FlaskConical aria-hidden className="h-4 w-4" />
            Test print
          </Button>
          <p className="mt-3 text-xs text-muted">
            Uses the selected printer and portrait paper. The print is not journaled.
          </p>
        </Card>
      </div>
    </div>
  );
}
