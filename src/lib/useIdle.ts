import { useEffect, useRef } from "react";

export function useIdle(timeoutMs: number, onIdle: () => void, resetKey: unknown): void {
  const callback = useRef(onIdle);
  callback.current = onIdle;

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    let disposed = false;

    const arm = () => {
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => {
        if (!disposed) callback.current();
      }, timeoutMs);
    };

    const onActivity = () => arm();

    const events: Array<keyof WindowEventMap> = [
      "keydown",
      "pointerdown",
      "mousemove",
      "wheel",
      "touchstart",
    ];
    for (const event of events) {
      window.addEventListener(event, onActivity, { passive: true });
    }
    arm();

    return () => {
      disposed = true;
      if (timer) clearTimeout(timer);
      for (const event of events) {
        window.removeEventListener(event, onActivity);
      }
    };
  }, [timeoutMs, resetKey]);
}
