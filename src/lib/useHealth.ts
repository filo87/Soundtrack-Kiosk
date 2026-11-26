import { useEffect, useState } from "react";
import { onHealth } from "./ipc";
import type { Health } from "./types";

export function useHealth(): Health | null {
  const [health, setHealth] = useState<Health | null>(null);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    onHealth((payload) => {
      if (!cancelled) setHealth(payload);
    })
      .then((fn) => {
        if (cancelled) {
          void Promise.resolve(fn()).catch(() => undefined);
        } else {
          unlisten = () => {
            void Promise.resolve(fn()).catch(() => undefined);
          };
        }
      })
      .catch(() => {
        // Health events are optional outside the kiosk shell.
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return health;
}
