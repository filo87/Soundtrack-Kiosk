import { useState, type FormEvent, type KeyboardEvent } from "react";

interface ManualEntryProps {
  onSubmit: (title: string, artist: string) => void;
  onBack: () => void;
}

export function ManualEntry({ onSubmit, onBack }: ManualEntryProps) {
  const [title, setTitle] = useState("");
  const [artist, setArtist] = useState("");

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (title.trim() && artist.trim()) {
      onSubmit(title, artist);
    }
  };

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
      event.preventDefault();
      onBack();
    }
  };

  return (
    <form onSubmit={submit} onKeyDown={onKeyDown} className="mt-8 space-y-5">
      <p className="text-lg text-muted">
        Spotify is unavailable right now. Type the song and artist instead.
      </p>
      <label className="block">
        <span className="mb-2 block text-sm font-medium text-muted">Song</span>
        <input
          autoFocus
          aria-label="Song"
          value={title}
          onChange={(event) => setTitle(event.target.value)}
          maxLength={200}
          className="w-full rounded-xl border border-steel/60 bg-sand/40 px-5 py-4 text-2xl text-ink outline-none focus:border-cerulean"
        />
      </label>
      <label className="block">
        <span className="mb-2 block text-sm font-medium text-muted">Artist</span>
        <input
          aria-label="Artist"
          value={artist}
          onChange={(event) => setArtist(event.target.value)}
          maxLength={200}
          className="w-full rounded-xl border border-steel/60 bg-sand/40 px-5 py-4 text-2xl text-ink outline-none focus:border-cerulean"
        />
      </label>
      <button type="submit" className="sr-only">
        Choose this song
      </button>
      <p className="text-base text-muted">Enter ↵ to choose</p>
    </form>
  );
}
