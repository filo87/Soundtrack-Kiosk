import type { Track } from "../lib/types";
import placeholder from "../assets/placeholder.jpg";

interface TrackListProps {
  tracks: Track[];
  highlighted: number;
  onHighlight: (index: number) => void;
  onPick: (track: Track) => void;
}

export function TrackList({ tracks, highlighted, onHighlight, onPick }: TrackListProps) {
  return (
    <ul role="listbox" aria-label="Search results" className="mt-6 space-y-2">
      {tracks.map((track, index) => {
        const selected = index === highlighted;
        return (
          <li key={track.id} role="option" aria-selected={selected}>
            <button
              type="button"
              onClick={() => onPick(track)}
              onMouseEnter={() => onHighlight(index)}
              className={`flex w-full items-center gap-4 rounded-xl px-4 py-3 text-left transition-colors ${
                selected ? "bg-salmon/20 ring-2 ring-cerulean" : "hover:bg-cerulean/10"
              }`}
            >
              <img
                src={track.image64 ?? track.image640 ?? placeholder}
                alt=""
                className="h-16 w-16 shrink-0 rounded-lg object-cover"
              />
              <span className="min-w-0">
                <span className="block truncate text-xl text-ink">{track.title}</span>
                <span className="block truncate text-base text-muted">
                  {track.artists.join(", ")}
                </span>
              </span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}
