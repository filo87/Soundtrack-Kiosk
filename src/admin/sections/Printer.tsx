import { FlaskConical, Monitor, Smartphone } from "lucide-react";
import { ipc } from "../../lib/ipc";
import type { AdminController } from "../useAdmin";
import { Button, Card, Field, Segmented, Select, StatusChip } from "../ui";
import { printerTone } from "./shared";

export function PrinterSection({ admin }: { admin: AdminController }) {
  const { status, config, printers } = admin;
  if (!status || !config) return null;
  const { health } = status;
  const configured = config.printer;
  const missingConfigured = configured !== null && !printers.printers.includes(configured);

  return (
    <div className="space-y-5">
      <Card
        title="Printer"
        description="The queue is used for guest prints, reprints and test prints."
        actions={
          <StatusChip tone={printerTone(health.printer.state)}>{health.printer.state}</StatusChip>
        }
      >
        <div className="grid gap-4 md:grid-cols-2">
          <Field label="Queue">
            <Select
              value={configured ?? ""}
              onChange={(event) => admin.setField("printer", event.target.value || null)}
              aria-label="Printer"
            >
              <option value="">System default ({printers.default ?? "none"})</option>
              {printers.printers.map((printer) => (
                <option key={printer} value={printer}>
                  {printer}
                </option>
              ))}
              {missingConfigured ? (
                <option value={configured}>{configured} (not currently listed)</option>
              ) : null}
            </Select>
          </Field>
          <Field
            label="Default orientation"
            hint="Guests can still choose either orientation before printing."
          >
            <Segmented
              label="Default orientation"
              value={config.defaultOrientation}
              onChange={(orientation) => admin.setField("defaultOrientation", orientation)}
              options={[
                {
                  value: "portrait",
                  label: "Portrait",
                  icon: <Smartphone aria-hidden className="h-4 w-4" />,
                },
                {
                  value: "landscape",
                  label: "Landscape",
                  icon: <Monitor aria-hidden className="h-4 w-4" />,
                },
              ]}
            />
          </Field>
        </div>
        <div className="mt-4 flex flex-wrap items-center gap-3">
          <Button
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
          <p className="text-xs text-muted">
            Test prints use the selected queue and portrait size.
          </p>
        </div>
      </Card>

      <Card title="Backend and queue">
        <dl className="grid gap-4 sm:grid-cols-3">
          <div className="rounded-xl bg-wheat/60 p-3.5">
            <dt className="text-xs uppercase tracking-wide text-muted">Backend</dt>
            <dd className="mt-1">
              <StatusChip tone="neutral">{status.printBackend}</StatusChip>
            </dd>
          </div>
          <div className="rounded-xl bg-wheat/60 p-3.5">
            <dt className="text-xs uppercase tracking-wide text-muted">Jobs waiting</dt>
            <dd className="mt-1 text-lg text-ink">{health.printsWaiting}</dd>
          </div>
          <div className="rounded-xl bg-wheat/60 p-3.5">
            <dt className="text-xs uppercase tracking-wide text-muted">Reason</dt>
            <dd className="mt-1 text-sm text-ink">{health.printer.reason ?? "None"}</dd>
          </div>
        </dl>
        <p className="mt-4 text-xs text-muted">
          A selected queue prints through CUPS with <code>lp</code>. With no queue selected, debug
          builds use the mock backend; release builds fall back to the CUPS default.
        </p>
      </Card>
    </div>
  );
}
