import { useId, useState } from "react";
import { Check, Circle, Eye, EyeOff, KeyRound, Power } from "lucide-react";
import { ipc } from "../../lib/ipc";
import type { AdminController } from "../useAdmin";
import { Button, Card, ConfirmDialog, Field, StatusChip, Switch, TextInput } from "../ui";

export function SecuritySection({ admin }: { admin: AdminController }) {
  const { status, config } = admin;
  const [showPin, setShowPin] = useState(false);
  const [confirmExit, setConfirmExit] = useState(false);
  const ruleId = useId();
  if (!status || !config) return null;

  const pin = config.pin;
  const digits = /^[0-9]{4,12}$/.test(pin);
  const notDefault = pin !== "0000";
  const matches = !admin.pinChanged || (admin.pinConfirm.length > 0 && admin.pinConfirm === pin);
  const rules = [
    { label: "4-12 digits", ok: digits },
    { label: "Not 0000", ok: notDefault },
    { label: "New PIN and confirmation match", ok: matches },
  ];
  const confirmError =
    admin.pinChanged && admin.pinConfirm.length > 0 && admin.pinConfirm !== pin
      ? "PINs do not match."
      : null;

  return (
    <div className="space-y-5">
      <Card
        title="Admin PIN"
        description="The PIN unlocks this overlay. The backend session expires after 5 minutes idle."
      >
        <div className="grid gap-4 md:grid-cols-2">
          <Field
            label="New PIN"
            hint="Leave unchanged to keep the current PIN."
            error={admin.fieldError("pin")}
          >
            <div className="flex gap-2">
              <TextInput
                type={showPin ? "text" : "password"}
                inputMode="numeric"
                autoComplete="new-password"
                maxLength={12}
                value={pin}
                onChange={(event) => admin.setField("pin", event.target.value)}
                onBlur={() => admin.markTouched("pin")}
                aria-label="New PIN"
                aria-invalid={Boolean(admin.fieldError("pin"))}
              />
              <Button
                type="button"
                onClick={() => setShowPin((value) => !value)}
                aria-label={showPin ? "Hide PIN" : "Show PIN"}
                className="shrink-0"
              >
                {showPin ? (
                  <EyeOff aria-hidden className="h-4 w-4" />
                ) : (
                  <Eye aria-hidden className="h-4 w-4" />
                )}
              </Button>
            </div>
          </Field>
          <Field label="Confirm new PIN" error={confirmError}>
            <TextInput
              type={showPin ? "text" : "password"}
              inputMode="numeric"
              autoComplete="new-password"
              maxLength={12}
              value={admin.pinConfirm}
              onChange={(event) => admin.setPinConfirm(event.target.value)}
              aria-label="Confirm new PIN"
              aria-invalid={Boolean(confirmError)}
            />
          </Field>
        </div>
        <ul id={ruleId} className="mt-4 grid gap-2 text-sm sm:grid-cols-3">
          {rules.map((rule) => (
            <li key={rule.label} className="flex items-center gap-2">
              {rule.ok ? (
                <Check aria-hidden className="h-4 w-4 text-cerulean" />
              ) : (
                <Circle aria-hidden className="h-4 w-4 text-steel" />
              )}
              <span className={rule.ok ? "text-ink" : "text-muted"}>{rule.label}</span>
            </li>
          ))}
        </ul>
        {pin === "0000" ? (
          <div className="mt-4 flex items-center gap-2 rounded-xl bg-salmon/20 p-3 text-sm text-ink">
            <KeyRound aria-hidden className="h-4 w-4" />
            Default PIN in use. Set a new PIN before the event.
          </div>
        ) : null}
      </Card>

      <Card
        title="Maintenance mode"
        description="Leaves kiosk mode for browser or diagnostics work."
        actions={
          <StatusChip tone={status.maintenance ? "attention" : "ok"}>
            {status.maintenance ? "On" : "Off"}
          </StatusChip>
        }
      >
        <div className="flex items-center justify-between gap-4 rounded-xl bg-wheat/60 p-4">
          <div>
            <p className="font-medium text-ink">Maintenance mode</p>
            <p className="mt-0.5 text-sm text-muted">
              The guest screen stays hidden until maintenance is turned off.
            </p>
          </div>
          <Switch
            checked={status.maintenance}
            disabled={admin.isPending("maintenance")}
            onChange={(enabled) =>
              void admin.run("maintenance", () => ipc.setMaintenance(enabled), {
                success: enabled ? "Maintenance mode enabled." : "Maintenance mode disabled.",
              })
            }
            label="Maintenance mode"
          />
        </div>
      </Card>

      <Card
        title="Danger zone"
        description="Actions that affect the whole kiosk."
        className="border-salmon/70 bg-salmon/10"
      >
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div>
            <p className="font-medium text-ink">Exit kiosk</p>
            <p className="mt-0.5 text-sm text-muted">
              Closes the app and restores the desktop. launchd does not reopen after a clean exit.
            </p>
          </div>
          <Button variant="danger" onClick={() => setConfirmExit(true)}>
            <Power aria-hidden className="h-4 w-4" />
            Exit kiosk
          </Button>
        </div>
      </Card>

      {confirmExit ? (
        <ConfirmDialog
          title="Exit kiosk?"
          confirmLabel="Exit kiosk"
          danger
          pending={admin.isPending("exit-kiosk")}
          onCancel={() => setConfirmExit(false)}
          onConfirm={() =>
            void admin.run("exit-kiosk", () => ipc.exitKiosk(), {
              onSuccess: () => setConfirmExit(false),
            })
          }
        >
          The guest screen closes and the kiosk stops. If it was started by launchd it stays closed
          after a clean exit. Use this only when the event is over.
        </ConfirmDialog>
      ) : null}
    </div>
  );
}
