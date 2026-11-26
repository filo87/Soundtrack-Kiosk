import { useEffect, useId, useState, type FormEvent } from "react";
import { motion, useReducedMotion } from "framer-motion";
import { LockKeyhole } from "lucide-react";
import { toKioskError } from "../lib/ipc";
import { Button } from "./ui";

interface PinPadProps {
  onUnlock: (pin: string) => Promise<void>;
  onClose: () => void;
}

export default function PinPad({ onUnlock, onClose }: PinPadProps) {
  const [pin, setPin] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [shake, setShake] = useState(false);
  const reduced = useReducedMotion();
  const titleId = useId();

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!pin || busy) return;
    setBusy(true);
    setError("");
    try {
      await onUnlock(pin);
    } catch (caught) {
      const kioskError = toKioskError(caught);
      setError(kioskError.message);
      setPin("");
      setShake(true);
    } finally {
      setBusy(false);
    }
  };

  const slots = Math.max(4, Math.min(12, pin.length));

  return (
    <div className="absolute inset-0 z-30 flex items-center justify-center bg-ink/55 p-6">
      <motion.form
        onSubmit={submit}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        initial={{ opacity: 0, scale: reduced ? 1 : 0.98 }}
        animate={
          shake && !reduced
            ? { opacity: 1, scale: 1, x: [0, -10, 10, -7, 7, -3, 3, 0] }
            : { opacity: 1, scale: 1, x: 0 }
        }
        transition={{ duration: shake && !reduced ? 0.4 : 0.15 }}
        onAnimationComplete={() => {
          if (shake) setShake(false);
        }}
        className="w-full max-w-md rounded-3xl border border-steel/60 bg-wheat p-8 shadow-2xl"
      >
        <div className="flex flex-col items-center text-center">
          <span className="rounded-2xl bg-cerulean/15 p-3">
            <LockKeyhole aria-hidden className="h-7 w-7 text-cerulean" />
          </span>
          <h2 id={titleId} className="mt-4 text-2xl font-medium text-ink">
            Staff access
          </h2>
          <p className="mt-1 text-sm text-muted">Enter the admin PIN to continue.</p>
        </div>

        <div aria-hidden className="mt-7 flex justify-center gap-2.5">
          {Array.from({ length: slots }).map((_, index) => (
            <span
              key={index}
              className={`h-3 w-3 rounded-full transition ${
                index < pin.length ? "bg-ink" : "border border-steel bg-sand/40"
              }`}
            />
          ))}
        </div>

        <label className="mt-5 block">
          <span className="sr-only">PIN</span>
          <input
            autoFocus
            type="password"
            inputMode="numeric"
            pattern="[0-9]*"
            aria-label="PIN"
            value={pin}
            onChange={(event) => {
              setPin(event.target.value.replace(/\D/g, ""));
              setError("");
            }}
            maxLength={12}
            className="w-full rounded-xl border border-steel/70 bg-sand/40 px-4 py-3 text-center text-2xl tracking-[0.5em] text-ink outline-none transition focus:border-cerulean focus:ring-2 focus:ring-cerulean/25"
          />
        </label>

        {error ? (
          <p
            role="alert"
            className="mt-3 flex items-start gap-2 rounded-xl bg-salmon/25 px-3 py-2 text-sm font-medium text-ink"
          >
            <span aria-hidden className="mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full bg-salmon" />
            {error}
          </p>
        ) : null}

        <div className="mt-6 flex justify-end gap-3">
          <Button type="button" variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" variant="primary" pending={busy} disabled={pin.length < 4}>
            Unlock
          </Button>
        </div>

        <p className="mt-4 text-center text-xs text-muted">Enter submits · Esc cancels</p>
      </motion.form>
    </div>
  );
}
