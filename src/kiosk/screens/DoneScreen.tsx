import { useEffect } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { Orientation } from "../../lib/types";
import placeholder from "../../assets/placeholder.jpg";

interface DoneScreenProps {
  active: boolean;
  guestName: string;
  orientation: Orientation;
  printFile: string | null;
  printsAhead: number;
  onNext: () => void;
}

export function DoneScreen({
  active,
  guestName,
  orientation,
  printFile,
  printsAhead,
  onNext,
}: DoneScreenProps) {
  useEffect(() => {
    if (!active) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Enter") {
        event.preventDefault();
        onNext();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [active, onNext]);

  const frame = orientation === "portrait" ? "aspect-[2/3]" : "aspect-[3/2]";
  const waiting = printsAhead > 0 ? ` after ${printsAhead} other print${printsAhead === 1 ? "" : "s"}` : "";

  return (
    <section className="flex w-full max-w-[960px] items-center gap-12">
      <div className={`${frame} w-[220px] shrink-0 overflow-hidden rounded-xl bg-white shadow-sm`}>
        <img
          src={printFile ? convertFileSrc(printFile) : placeholder}
          alt="Your print"
          onError={(event) => {
            event.currentTarget.onerror = null;
            event.currentTarget.src = placeholder;
          }}
          className="h-full w-full object-contain"
        />
      </div>
      <div>
        <h1 className="text-5xl font-medium leading-tight text-ink">Thank you, {guestName}!</h1>
        <p className="mt-6 max-w-[520px] text-2xl leading-relaxed text-muted">
          Your print is on its way — collect it at the printer in about a minute{waiting}.
        </p>
        <p className="mt-3 max-w-[520px] text-xl leading-relaxed text-ink">
          Stick it in the album and add a memory or a wish.
        </p>
        <p className="mt-8 text-base text-muted">Enter ↵ for the next guest</p>
      </div>
    </section>
  );
}
