import type { FormEvent, KeyboardEvent } from "react";

interface NameScreenProps {
  welcome: string;
  name: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
}

export function NameScreen({ welcome, name, onChange, onSubmit }: NameScreenProps) {
  const submit = (event: FormEvent) => {
    event.preventDefault();
    onSubmit();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      onChange("");
    }
  };

  return (
    <section className="w-full max-w-[960px]">
      <p className="text-xl tracking-wide text-muted">{welcome}</p>
      <h1 className="mt-4 text-6xl font-medium leading-tight text-ink">What&rsquo;s your name?</h1>
      <form onSubmit={submit} className="mt-12">
        <input
          autoFocus
          aria-label="Your name"
          value={name}
          onChange={(event) => onChange(event.target.value)}
          onKeyDown={onKeyDown}
          maxLength={40}
          className="w-full border-b-2 border-steel/70 bg-transparent pb-4 text-5xl text-ink outline-none focus:border-cerulean"
        />
        <p className="mt-6 text-lg text-muted">Enter ↵</p>
      </form>
    </section>
  );
}
