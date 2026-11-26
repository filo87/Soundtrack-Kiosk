import {
  forwardRef,
  useEffect,
  useId,
  useRef,
  useState,
  type ButtonHTMLAttributes,
  type InputHTMLAttributes,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
  type SelectHTMLAttributes,
} from "react";
import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import { Check, Loader2, X } from "lucide-react";

export function Card({
  title,
  description,
  actions,
  children,
  className = "",
}: {
  title?: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={`rounded-2xl border border-steel/60 bg-sand/25 p-5 ${className}`}>
      {title || description || actions ? (
        <div className="mb-4 flex items-start justify-between gap-4">
          <div>
            {title ? <h3 className="text-lg font-medium text-ink">{title}</h3> : null}
            {description ? <p className="mt-1 text-sm text-muted">{description}</p> : null}
          </div>
          {actions ? <div className="flex shrink-0 items-center gap-2">{actions}</div> : null}
        </div>
      ) : null}
      {children}
    </section>
  );
}

export function Field({
  label,
  hint,
  error,
  children,
}: {
  label: ReactNode;
  hint?: ReactNode;
  error?: string | null;
  children: ReactNode;
}) {
  return (
    <label className="block text-sm">
      <span className="mb-1.5 block font-medium text-ink">{label}</span>
      {children}
      {hint && !error ? <span className="mt-1.5 block text-xs text-muted">{hint}</span> : null}
      {error ? (
        <span className="mt-1.5 flex items-start gap-1.5 rounded-lg bg-salmon/25 px-2.5 py-1.5 text-xs font-medium text-ink">
          <span aria-hidden className="mt-1 h-1.5 w-1.5 shrink-0 rounded-full bg-salmon" />
          {error}
        </span>
      ) : null}
    </label>
  );
}

const inputClass =
  "w-full rounded-xl border border-steel/70 bg-sand/40 px-3 py-2.5 text-ink outline-none transition focus:border-cerulean focus:ring-2 focus:ring-cerulean/25 disabled:cursor-not-allowed disabled:opacity-60";

export const TextInput = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(
  function TextInput({ className = "", ...rest }, ref) {
    return <input ref={ref} {...rest} className={`${inputClass} ${className}`} />;
  },
);

export function Select(props: SelectHTMLAttributes<HTMLSelectElement>) {
  const { className = "", ...rest } = props;
  return <select {...rest} className={`${inputClass} ${className}`} />;
}

export interface SegmentedOption<T extends string> {
  value: T;
  label: string;
  icon?: ReactNode;
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: SegmentedOption<T>[];
  onChange: (value: T) => void;
  label: string;
}) {
  return (
    <div
      role="group"
      aria-label={label}
      className="inline-flex rounded-xl border border-steel/70 bg-sand/30 p-1"
    >
      {options.map((option) => {
        const selected = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            aria-pressed={selected}
            onClick={() => onChange(option.value)}
            className={`flex items-center gap-2 rounded-lg px-4 py-2 text-sm font-medium transition ${
              selected
                ? "bg-cerulean/15 text-ink shadow-[inset_0_-2px_0_0_var(--color-cerulean)]"
                : "text-muted hover:bg-wheat/50 hover:text-ink"
            }`}
          >
            {option.icon}
            {option.label}
          </button>
        );
      })}
    </div>
  );
}

export function Switch({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative h-7 w-12 shrink-0 rounded-full border transition ${
        checked ? "border-cerulean bg-cerulean/25" : "border-steel/70 bg-steel/30"
      } disabled:cursor-not-allowed disabled:opacity-50`}
    >
      <span
        aria-hidden
        className={`absolute top-0.5 h-5 w-5 rounded-full shadow transition-all ${
          checked ? "left-[26px] bg-cerulean" : "left-0.5 bg-wheat"
        }`}
      />
    </button>
  );
}

type ButtonVariant = "primary" | "secondary" | "danger" | "ghost";

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant;
  size?: "sm" | "md";
  pending?: boolean;
};

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  {
    children,
    variant = "secondary",
    size = "md",
    pending = false,
    className = "",
    disabled,
    ...rest
  },
  ref,
) {
  const variants: Record<ButtonVariant, string> = {
    primary: "border border-transparent bg-salmon text-ink hover:bg-salmon/90",
    secondary: "border border-steel/70 bg-wheat/60 text-ink hover:bg-cerulean/10",
    danger: "border border-transparent bg-ink text-wheat hover:bg-ink/90",
    ghost: "border border-transparent text-muted hover:bg-cerulean/10 hover:text-ink",
  };
  const sizes = {
    sm: "px-3 py-1.5 text-xs",
    md: "px-4 py-2 text-sm",
  };
  return (
    <button
      {...rest}
      ref={ref}
      disabled={disabled || pending}
      className={`inline-flex items-center justify-center gap-2 rounded-xl font-medium transition disabled:cursor-not-allowed disabled:opacity-50 ${variants[variant]} ${sizes[size]} ${className}`}
    >
      {pending ? <Loader2 aria-hidden className="h-4 w-4 animate-spin" /> : null}
      {children}
    </button>
  );
});

export type ChipTone = "ok" | "attention" | "neutral" | "active";

export function StatusChip({
  tone = "neutral",
  children,
}: {
  tone?: ChipTone;
  children: ReactNode;
}) {
  const tones: Record<ChipTone, string> = {
    ok: "bg-cerulean/15 text-ink",
    attention: "bg-salmon/25 text-ink",
    neutral: "bg-steel/20 text-muted",
    active: "bg-ink text-wheat",
  };
  const dots: Record<ChipTone, string> = {
    ok: "bg-cerulean",
    attention: "bg-salmon",
    neutral: "bg-steel",
    active: "bg-wheat",
  };
  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-medium ${tones[tone]}`}
    >
      <span aria-hidden className={`h-1.5 w-1.5 rounded-full ${dots[tone]}`} />
      {children}
    </span>
  );
}

export function ProgressBar({ value, max, label }: { value: number; max: number; label: string }) {
  const pct = max > 0 ? Math.min(100, Math.round((value / max) * 100)) : 0;
  const current = Math.max(0, Math.min(value, max));
  return (
    <div
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={max}
      aria-valuenow={current}
      className="h-2.5 w-full overflow-hidden rounded-full bg-steel/30"
    >
      <div
        className={`h-full rounded-full transition-all ${pct >= 80 ? "bg-salmon" : "bg-cerulean"}`}
        style={{ width: `${pct}%` }}
      />
    </div>
  );
}

export interface ToastItem {
  id: number;
  kind: "success" | "error" | "info";
  message: string;
}

export function ToastStack({
  toasts,
  onDismiss,
}: {
  toasts: ToastItem[];
  onDismiss: (id: number) => void;
}) {
  return (
    <div
      aria-live="polite"
      aria-relevant="additions text"
      className="pointer-events-none absolute bottom-24 right-6 z-40 flex w-[min(28rem,calc(100%-3rem))] flex-col gap-2"
    >
      <AnimatePresence initial={false}>
        {toasts.map((toast) => (
          <motion.div
            key={toast.id}
            role={toast.kind === "error" ? "alert" : "status"}
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 8 }}
            transition={{ duration: 0.15 }}
            className={`pointer-events-auto flex items-start gap-3 rounded-xl border px-4 py-3 text-sm shadow-lg ${
              toast.kind === "error"
                ? "border-salmon/70 bg-salmon/25 text-ink"
                : "border-steel/60 bg-wheat text-ink"
            }`}
          >
            {toast.kind === "success" ? (
              <Check aria-hidden className="mt-0.5 h-4 w-4 shrink-0 text-cerulean" />
            ) : (
              <span
                aria-hidden
                className={`mt-1.5 h-2 w-2 shrink-0 rounded-full ${
                  toast.kind === "error" ? "bg-salmon" : "bg-steel"
                }`}
              />
            )}
            <span className="min-w-0 flex-1">{toast.message}</span>
            <button
              type="button"
              aria-label="Dismiss notification"
              onClick={() => onDismiss(toast.id)}
              className="rounded-md p-0.5 text-muted hover:bg-cerulean/10 hover:text-ink"
            >
              <X aria-hidden className="h-4 w-4" />
            </button>
          </motion.div>
        ))}
      </AnimatePresence>
    </div>
  );
}

export function ConfirmDialog({
  title,
  children,
  confirmLabel,
  cancelLabel = "Cancel",
  danger = false,
  pending = false,
  requireText,
  onConfirm,
  onCancel,
}: {
  title: string;
  children: ReactNode;
  confirmLabel: string;
  cancelLabel?: string;
  danger?: boolean;
  pending?: boolean;
  requireText?: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const titleId = useId();
  const reduced = useReducedMotion();
  const [text, setText] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const blocked = Boolean(requireText) && text.trim() !== requireText;

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    (requireText ? inputRef.current : cancelRef.current)?.focus();
    return () => previous?.focus?.();
  }, [requireText]);

  const onKeyDown = (event: ReactKeyboardEvent) => {
    if (event.key !== "Escape" || pending) return;
    event.preventDefault();
    event.stopPropagation();
    onCancel();
  };

  return (
    <div className="absolute inset-0 z-50 flex items-center justify-center bg-ink/55 p-6">
      <motion.div
        role="alertdialog"
        aria-modal="true"
        aria-labelledby={titleId}
        onKeyDown={onKeyDown}
        initial={{ opacity: 0, y: reduced ? 0 : 8, scale: reduced ? 1 : 0.98 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        transition={{ duration: reduced ? 0 : 0.15 }}
        className="w-full max-w-lg rounded-2xl border border-steel/60 bg-wheat p-6 shadow-2xl"
      >
        <h3 id={titleId} className="text-xl font-medium text-ink">
          {title}
        </h3>
        <div className="mt-2 text-sm text-muted">{children}</div>
        {requireText ? (
          <label className="mt-4 block text-sm">
            <span className="mb-1.5 block font-medium text-ink">
              Type{" "}
              <span className="rounded bg-ink px-1.5 py-0.5 font-mono text-wheat">
                {requireText}
              </span>{" "}
              to continue
            </span>
            <TextInput
              ref={inputRef}
              value={text}
              onChange={(event) => setText(event.target.value)}
              aria-label={`Type ${requireText} to confirm`}
              autoComplete="off"
            />
          </label>
        ) : null}
        <div className="mt-6 flex justify-end gap-3">
          <Button ref={cancelRef} variant="ghost" onClick={onCancel} disabled={pending}>
            {cancelLabel}
          </Button>
          <Button
            variant={danger ? "danger" : "primary"}
            onClick={onConfirm}
            pending={pending}
            disabled={blocked}
          >
            {confirmLabel}
          </Button>
        </div>
      </motion.div>
    </div>
  );
}
