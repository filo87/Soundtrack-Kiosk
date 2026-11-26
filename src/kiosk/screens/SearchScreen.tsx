import type { KeyboardEvent } from "react";
import type { Track } from "../../lib/types";
import { ManualEntry } from "../ManualEntry";
import { TrackList } from "../TrackList";

interface SearchScreenProps {
  guestName: string;
  babyName: string;
  query: string;
  results: Track[];
  degraded: boolean;
  highlighted: number;
  onQueryChange: (query: string) => void;
  onMove: (delta: number) => void;
  onHighlight: (index: number) => void;
  onPick: (track: Track) => void;
  onManual: (title: string, artist: string) => void;
  onBack: () => void;
}

export function SearchScreen({
  guestName,
  babyName,
  query,
  results,
  degraded,
  highlighted,
  onQueryChange,
  onMove,
  onHighlight,
  onPick,
  onManual,
  onBack,
}: SearchScreenProps) {
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      onMove(1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      onMove(-1);
    } else if (event.key === "Enter") {
      event.preventDefault();
      const track = results[highlighted];
      if (!degraded && track) onPick(track);
    } else if (event.key === "Escape") {
      event.preventDefault();
      onBack();
    }
  };

  const tooShort = query.trim().length < 2;

  return (
    <section className="w-full max-w-[960px]">
      <h1 className="text-4xl font-medium leading-tight text-ink">
        Hi {guestName}, which song should we play for {babyName}?
      </h1>
      <input
        autoFocus
        aria-label="Search songs"
        value={query}
        onChange={(event) => onQueryChange(event.target.value)}
        onKeyDown={onKeyDown}
        maxLength={120}
        placeholder="Song or artist"
        className="mt-8 w-full rounded-xl border border-steel/60 bg-sand/40 px-5 py-4 text-2xl text-ink outline-none focus:border-cerulean"
      />

      {degraded ? (
        <ManualEntry onSubmit={onManual} onBack={onBack} />
      ) : (
        <>
          {tooShort ? (
            <p className="mt-6 text-lg text-muted">Type at least 2 characters.</p>
          ) : null}
          {!tooShort && results.length === 0 ? (
            <p className="mt-6 text-lg text-muted">No songs found, try the artist&rsquo;s name.</p>
          ) : null}
          <TrackList
            tracks={results}
            highlighted={highlighted}
            onHighlight={onHighlight}
            onPick={onPick}
          />
          {results.length > 0 ? (
            <p className="mt-6 text-base text-muted">↑ ↓ to browse · Enter ↵ to choose · Esc to go back</p>
          ) : null}
        </>
      )}
    </section>
  );
}
