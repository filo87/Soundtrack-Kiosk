export type Orientation = "portrait" | "landscape";
export type PrintLayout = "cover" | "fit";

export interface Track {
  id: string;
  uri: string;
  title: string;
  artists: string[];
  album: string;
  albumId?: string | null;
  image640?: string | null;
  image64?: string | null;
}

export interface SearchResponse {
  tracks: Track[];
  degraded: boolean;
}

export type SelectionInput =
  { kind: "track"; track: Track } | { kind: "manual"; title: string; artist: string };

export interface PreviewPaths {
  portraitCover: string;
  portraitFit: string;
  landscapeCover: string;
  landscapeFit: string;
}

export interface PrepareResult {
  prepId: string;
  previews: PreviewPaths;
  coverSource: string;
  coverPx: number;
}

export interface CommitResult {
  printsAhead: number;
  printQueued: boolean;
  printFile: string | null;
}

export interface PrinterHealth {
  state: string;
  reason: string | null;
}

export interface Health {
  auth: boolean;
  network: boolean;
  printer: PrinterHealth;
  printsWaiting: number;
  outboxPending: number;
  refillDue: boolean;
  paperLimit: number;
  inkLimit: number;
  attention: boolean;
}

export interface PublicConfig {
  babyName: string;
  welcome: string;
  defaultOrientation: Orientation;
}

export interface Config {
  babyName: string;
  welcomeLine: string;
  playlistUrl: string;
  market: string;
  country: string;
  defaultOrientation: Orientation;
  printer: string | null;
  pin: string;
  spotifyClientId: string;
  paperCount: number;
  inkCount: number;
}

export interface RecentSelection {
  id: string;
  guestName: string;
  trackTitle: string;
  artist: string;
  timestamp: string;
  orientation: string;
  layout: string;
  mode: string;
  trackUri: string | null;
  printState: string;
  printFile: string | null;
  printReason: string | null;
  playlistSkipped: boolean;
  playlistSkipReason: string | null;
}

export interface AdminStatus {
  health: Health;
  config: Config;
  printBackend: string;
  maintenance: boolean;
  spotifyRedirectUri: string;
  recent: RecentSelection[];
  unprinted: RecentSelection[];
}

export interface ClearHistoryResult {
  archived: number;
  pendingArchived: number;
  archivePath: string | null;
}

export interface PrinterList {
  printers: string[];
  default: string | null;
}

export interface PrintOutcome {
  job: string;
  file: string;
}
