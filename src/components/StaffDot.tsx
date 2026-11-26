export function StaffDot({ attention }: { attention: boolean }) {
  if (!attention) return null;
  return (
    <span
      aria-label="Staff attention needed"
      title="Staff attention needed"
      className="absolute right-4 top-4 z-40 h-2.5 w-2.5 rounded-full bg-salmon shadow ring-2 ring-ink/40"
    />
  );
}
