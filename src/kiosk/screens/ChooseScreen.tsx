import { useEffect, type ReactNode } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type {
  Orientation,
  PrepareResult,
  PrintLayout,
  SelectionInput,
} from "../../lib/types";
import placeholder from "../../assets/placeholder.jpg";

interface ChooseScreenProps {
  active: boolean;
  selection: SelectionInput;
  prep: PrepareResult | null;
  orientation: Orientation;
  layout: PrintLayout;
  onOrientation: (orientation: Orientation) => void;
  onLayout: (layout: PrintLayout) => void;
  onCommit: (orientation: Orientation, layout: PrintLayout) => void;
  onBack: () => void;
}

interface LayoutOption {
  orientation: Orientation;
  layout: PrintLayout;
  label: string;
  subtitle: string;
  preview: keyof PrepareResult["previews"];
}

const options: LayoutOption[] = [
  {
    orientation: "portrait",
    layout: "cover",
    label: "Portrait · Cover",
    subtitle: "full-bleed · no writing margin",
    preview: "portraitCover",
  },
  {
    orientation: "landscape",
    layout: "cover",
    label: "Landscape · Cover",
    subtitle: "full-bleed · no writing margin",
    preview: "landscapeCover",
  },
  {
    orientation: "portrait",
    layout: "fit",
    label: "Portrait · Fit",
    subtitle: "white margin for a memory or wish",
    preview: "portraitFit",
  },
  {
    orientation: "landscape",
    layout: "fit",
    label: "Landscape · Fit",
    subtitle: "white margin for a memory or wish",
    preview: "landscapeFit",
  },
];

export function ChooseScreen({
  active,
  selection,
  prep,
  orientation,
  layout,
  onOrientation,
  onLayout,
  onCommit,
  onBack,
}: ChooseScreenProps) {
  useEffect(() => {
    if (!active) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "ArrowLeft") {
        event.preventDefault();
        onOrientation("portrait");
      } else if (event.key === "ArrowRight") {
        event.preventDefault();
        onOrientation("landscape");
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        onLayout("cover");
      } else if (event.key === "ArrowDown") {
        event.preventDefault();
        onLayout("fit");
      } else if (event.key === "Enter") {
        event.preventDefault();
        if (prep) onCommit(orientation, layout);
      } else if (event.key === "Escape") {
        event.preventDefault();
        onBack();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [
    active,
    onOrientation,
    onLayout,
    onCommit,
    onBack,
    orientation,
    layout,
    prep,
  ]);

  const cover =
    selection.kind === "track"
      ? (selection.track.image640 ?? selection.track.image64 ?? placeholder)
      : placeholder;

  return (
    <section className="w-full max-w-[960px]">
      <h1 className="text-center text-4xl font-medium text-ink">Choose your print</h1>
      <div className="mt-8 grid grid-cols-2 gap-5">
        {options.map((option) => {
          const selected =
            orientation === option.orientation && layout === option.layout;
          const src = prep
            ? convertFileSrc(prep.previews[option.preview])
            : cover;
          return (
            <button
              key={option.label}
              type="button"
              aria-label={`Print ${option.label}`}
              onClick={() => onCommit(option.orientation, option.layout)}
              onMouseEnter={() => {
                onOrientation(option.orientation);
                onLayout(option.layout);
              }}
              className={`rounded-2xl p-3 text-left transition-shadow ${
                selected ? "bg-salmon/20 ring-2 ring-cerulean" : "bg-sand/40"
              }`}
            >
              <div className="flex h-[26vh] items-center justify-center overflow-hidden rounded-xl bg-steel/40 p-2">
                {prep ? (
                  <img
                    src={src}
                    alt={`${option.label} preview`}
                    className="max-h-full max-w-full rounded-sm object-contain shadow-sm"
                  />
                ) : (
                  <img
                    src={cover}
                    alt={`${option.label} cover`}
                    className="soft-pulse max-h-full max-w-full object-contain opacity-60"
                  />
                )}
              </div>
              <p
                className={`mt-2 text-center text-lg ${
                  selected ? "text-ink" : "text-muted"
                }`}
              >
                {option.label}
              </p>
              <p className="text-center text-sm text-muted">{option.subtitle}</p>
            </button>
          );
        })}
      </div>
      <Hint>
        {prep
          ? "← → portrait/landscape · ↑ ↓ cover/fit · Enter ↵ to print · Esc back"
          : "Preparing your prints…"}
      </Hint>
    </section>
  );
}

function Hint({ children }: { children: ReactNode }) {
  return <p className="mt-6 text-center text-sm text-muted">{children}</p>;
}
