import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ipc, toKioskError } from "../lib/ipc";
import type { AdminStatus, Config, PrinterList } from "../lib/types";
import {
  ADMIN_FIELDS,
  mapServerError,
  validateConfig,
  type AdminField,
  type FieldErrors,
} from "./validation";
import type { ToastItem } from "./ui";

const EMPTY_PRINTERS: PrinterList = { printers: [], default: null };

function mergeCounters(target: Config, source: Config): Config {
  return {
    ...target,
    paperCount: source.paperCount,
    inkCount: source.inkCount,
  };
}

function messageFor<T>(result: T, success?: string | ((value: T) => string)) {
  if (!success) return null;
  return typeof success === "function" ? success(result) : success;
}

export interface RunOptions<T> {
  success?: string | ((value: T) => string);
  onSuccess?: (value: T) => void;
  reload?: boolean;
}

export function useAdmin({ onClose }: { onClose: () => void }) {
  const [status, setStatus] = useState<AdminStatus | null>(null);
  const [config, setConfig] = useState<Config | null>(null);
  const [baseline, setBaseline] = useState<Config | null>(null);
  const [printers, setPrinters] = useState<PrinterList>(EMPTY_PRINTERS);
  const [pinConfirm, setPinConfirm] = useState("");
  const [touched, setTouched] = useState<Partial<Record<AdminField, boolean>>>({});
  const [serverErrors, setServerErrors] = useState<FieldErrors>({});
  const [submitAttempted, setSubmitAttempted] = useState(false);
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const [pending, setPending] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const toastId = useRef(0);
  const toastTimers = useRef(new Map<number, ReturnType<typeof setTimeout>>());

  const dismissToast = useCallback((id: number) => {
    const timer = toastTimers.current.get(id);
    if (timer) clearTimeout(timer);
    toastTimers.current.delete(id);
    setToasts((current) => current.filter((toast) => toast.id !== id));
  }, []);

  const pushToast = useCallback(
    (kind: ToastItem["kind"], message: string) => {
      const id = ++toastId.current;
      setToasts((current) => [...current.slice(-3), { id, kind, message }]);
      if (kind !== "error") {
        const timer = setTimeout(() => dismissToast(id), 4000);
        toastTimers.current.set(id, timer);
      }
    },
    [dismissToast],
  );

  useEffect(() => {
    const timers = toastTimers.current;
    return () => {
      for (const timer of timers.values()) clearTimeout(timer);
      timers.clear();
    };
  }, []);

  const fail = useCallback(
    (caught: unknown) => {
      const error = toKioskError(caught);
      if (error.code === "ADMIN_LOCKED") {
        onClose();
        return null;
      }
      return error;
    },
    [onClose],
  );

  const reload = useCallback(async () => {
    try {
      const next = await ipc.adminStatus();
      setStatus(next);
      setConfig((current) => (current ? mergeCounters(current, next.config) : next.config));
      setBaseline((current) => (current ? mergeCounters(current, next.config) : next.config));
      try {
        setPrinters(await ipc.listPrinters());
      } catch {
        setPrinters(EMPTY_PRINTERS);
      }
      return next;
    } catch (caught) {
      const error = fail(caught);
      if (error) pushToast("error", error.message);
      return null;
    }
  }, [fail, pushToast]);

  useEffect(() => {
    let active = true;
    void reload().finally(() => {
      if (active) setLoading(false);
    });
    return () => {
      active = false;
    };
  }, [reload]);

  const pinChanged = Boolean(config && baseline && config.pin !== baseline.pin);
  const errors = useMemo<FieldErrors>(
    () => (config ? validateConfig(config, pinChanged ? pinConfirm : undefined) : {}),
    [config, pinChanged, pinConfirm],
  );
  const valid = Object.keys(errors).length === 0;

  const dirty = useMemo(() => {
    if (!config || !baseline) return false;
    return ADMIN_FIELDS.some((field) => config[field] !== baseline[field]);
  }, [config, baseline]);

  const setField = useCallback(<K extends AdminField>(field: K, value: Config[K]) => {
    setConfig((current) => (current ? { ...current, [field]: value } : current));
    setServerErrors((current) => {
      if (!current[field]) return current;
      const next = { ...current };
      delete next[field];
      return next;
    });
  }, []);

  const markTouched = useCallback((field: AdminField) => {
    setTouched((current) => (current[field] ? current : { ...current, [field]: true }));
  }, []);

  const fieldError = useCallback(
    (field: AdminField) => {
      const local = errors[field];
      if (local && (touched[field] || submitAttempted)) return local;
      return serverErrors[field];
    },
    [errors, touched, submitAttempted, serverErrors],
  );

  const discard = useCallback(() => {
    setConfig(baseline);
    setTouched({});
    setServerErrors({});
    setSubmitAttempted(false);
    setPinConfirm("");
  }, [baseline]);

  const save = useCallback(async () => {
    if (!config) return false;
    setSubmitAttempted(true);
    if (!valid) {
      pushToast("error", "Fix the highlighted fields before saving.");
      return false;
    }
    setSaving(true);
    try {
      const saved = await ipc.setConfig(config);
      setConfig(saved);
      setBaseline(saved);
      setTouched({});
      setServerErrors({});
      setSubmitAttempted(false);
      setPinConfirm("");
      pushToast("success", "Settings saved.");
      void reload();
      return true;
    } catch (caught) {
      const error = fail(caught);
      if (!error) return false;
      const field = mapServerError(error.message);
      if (field) {
        setServerErrors((current) => ({ ...current, [field]: error.message }));
      } else {
        pushToast("error", error.message);
      }
      return false;
    } finally {
      setSaving(false);
    }
  }, [config, valid, fail, pushToast, reload]);

  const run = useCallback(
    async <T>(
      name: string,
      action: () => Promise<T>,
      options: RunOptions<T> = {},
    ): Promise<T | null> => {
      setPending((current) => (current.includes(name) ? current : [...current, name]));
      try {
        const result = await action();
        options.onSuccess?.(result);
        const message = messageFor(result, options.success);
        if (message) pushToast("success", message);
        if (options.reload !== false) await reload();
        return result;
      } catch (caught) {
        const error = fail(caught);
        if (error) pushToast("error", error.message);
        return null;
      } finally {
        setPending((current) => current.filter((item) => item !== name));
      }
    },
    [fail, pushToast, reload],
  );

  const isPending = useCallback((name: string) => pending.includes(name), [pending]);

  return {
    status,
    config,
    baseline,
    printers,
    loading,
    saving,
    dirty,
    valid,
    errors,
    fieldError,
    markTouched,
    setField,
    pinConfirm,
    setPinConfirm,
    pinChanged,
    save,
    discard,
    run,
    isPending,
    toasts,
    dismissToast,
    pushToast,
    reload,
    fail,
  };
}

export type AdminController = ReturnType<typeof useAdmin>;
