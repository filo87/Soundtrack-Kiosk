import type { AdminController } from "../useAdmin";
import { WELCOME_MAX } from "../validation";
import { Button, Card, Field, TextInput } from "../ui";

export function EventSection({ admin }: { admin: AdminController }) {
  const { config } = admin;
  if (!config) return null;
  const baby = config.babyName.trim() || "the baby";
  const preview = config.welcomeLine.replace(/\{babyName\}|\{name\}/g, baby);
  const insertToken = () => {
    const base = config.welcomeLine;
    const next = base && !base.endsWith(" ") ? `${base} {babyName}` : `${base}{babyName}`;
    admin.setField("welcomeLine", next);
  };

  return (
    <div className="space-y-5">
      <Card
        title="Event details"
        description="The baby name and welcome line appear on the Name screen."
      >
        <div className="grid gap-4 md:grid-cols-2">
          <Field label="Baby name" hint="Used in the welcome line and the guest prompt.">
            <TextInput
              value={config.babyName}
              onChange={(event) => admin.setField("babyName", event.target.value)}
              maxLength={40}
              autoComplete="off"
              aria-label="Baby name"
            />
          </Field>
          <div>
            <Field
              label="Welcome line"
              hint={`${config.welcomeLine.length} / ${WELCOME_MAX}`}
              error={admin.fieldError("welcomeLine")}
            >
              <TextInput
                value={config.welcomeLine}
                onChange={(event) => admin.setField("welcomeLine", event.target.value)}
                onBlur={() => admin.markTouched("welcomeLine")}
                maxLength={WELCOME_MAX}
                autoComplete="off"
                aria-label="Welcome line"
                aria-invalid={Boolean(admin.fieldError("welcomeLine"))}
              />
            </Field>
            <div className="mt-2 flex items-center gap-2 text-xs text-muted">
              <span>Insert</span>
              <Button type="button" size="sm" variant="secondary" onClick={insertToken}>
                <code className="font-mono">{`{babyName}`}</code>
              </Button>
              <span>at the end of the line.</span>
            </div>
          </div>
        </div>
      </Card>

      <Card title="Name screen preview" description="The first line guests read.">
        <div className="rounded-xl border border-steel/60 bg-wheat p-6">
          <p className="min-h-6 text-xl tracking-wide text-muted">{preview}</p>
          <p className="mt-3 text-4xl font-medium leading-tight text-ink">
            What&rsquo;s your name?
          </p>
          <p className="mt-4 text-sm text-muted">Enter ↵</p>
        </div>
        <p className="mt-3 text-xs text-muted">
          Use{" "}
          <code className="rounded bg-ink px-1 py-0.5 font-mono text-wheat">{`{babyName}`}</code> or{" "}
          <code className="rounded bg-ink px-1 py-0.5 font-mono text-wheat">{`{name}`}</code> in the
          welcome line to insert the baby name.
        </p>
      </Card>
    </div>
  );
}
