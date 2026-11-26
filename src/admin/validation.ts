import type { Config } from "../lib/types";

export const WELCOME_MAX = 80;

export type AdminField =
  | "babyName"
  | "welcomeLine"
  | "playlistUrl"
  | "market"
  | "country"
  | "defaultOrientation"
  | "printer"
  | "pin"
  | "spotifyClientId";

export type FieldErrors = Partial<Record<AdminField, string>>;

export const ADMIN_FIELDS: AdminField[] = [
  "babyName",
  "welcomeLine",
  "playlistUrl",
  "market",
  "country",
  "defaultOrientation",
  "printer",
  "pin",
  "spotifyClientId",
];

/**
 * Mirrors `config::parse_playlist_id` in the Rust backend: `spotify:playlist:{id}`,
 * an absolute Spotify playlist URL, or a bare id of at least 10 alphanumeric characters.
 */
export function parsePlaylistId(input: string): string | null {
  const trimmed = input.trim();
  if (!trimmed) return null;

  let candidate: string;
  if (trimmed.startsWith("spotify:playlist:")) {
    const rest = trimmed.slice("spotify:playlist:".length);
    candidate = rest.split("?")[0] ?? "";
  } else {
    const parsed = tryParseUrl(trimmed);
    if (parsed) {
      const segments = parsed.pathname.split("/").filter(Boolean);
      const index = segments.indexOf("playlist");
      candidate = index >= 0 ? (segments[index + 1] ?? "") : "";
    } else {
      candidate = trimmed;
    }
  }

  const id = candidate.match(/^[A-Za-z0-9]+/)?.[0] ?? "";
  return id.length >= 10 ? id : null;
}

function tryParseUrl(value: string): URL | null {
  try {
    return new URL(value);
  } catch {
    return null;
  }
}

export function validatePin(pin: string): string | null {
  if (!/^[0-9]{4,12}$/.test(pin)) return "Use 4-12 digits.";
  if (pin === "0000") return "Choose a PIN other than 0000.";
  return null;
}

export function validatePinConfirm(pin: string, confirm: string): string | null {
  if (confirm !== pin) return "PINs do not match.";
  return null;
}

export function validatePlaylistUrl(value: string): string | null {
  if (!value.trim()) return null;
  return parsePlaylistId(value) ? null : "That playlist URL or id is not valid.";
}

export function validateClientId(value: string): string | null {
  if (!value.trim()) return null;
  return /^[0-9a-fA-F]{32}$/.test(value) ? null : "A Spotify client id is 32 hex characters.";
}

export function validateRegionCode(value: string): string | null {
  return /^[A-Za-z]{2}$/.test(value) ? null : "Use a 2-letter code, e.g. US.";
}

export function validateWelcomeLine(value: string): string | null {
  return value.length > WELCOME_MAX ? `Keep the welcome line to ${WELCOME_MAX} characters.` : null;
}

export function validateConfig(config: Pick<Config, AdminField>, pinConfirm?: string): FieldErrors {
  const errors: FieldErrors = {};
  const playlist = validatePlaylistUrl(config.playlistUrl);
  if (playlist) errors.playlistUrl = playlist;
  const clientId = validateClientId(config.spotifyClientId);
  if (clientId) errors.spotifyClientId = clientId;
  const market = validateRegionCode(config.market);
  if (market) errors.market = market;
  const country = validateRegionCode(config.country);
  if (country) errors.country = country;
  const welcome = validateWelcomeLine(config.welcomeLine);
  if (welcome) errors.welcomeLine = welcome;
  const pin = validatePin(config.pin);
  if (pin) errors.pin = pin;
  else if (pinConfirm !== undefined) {
    const confirm = validatePinConfirm(config.pin, pinConfirm);
    if (confirm) errors.pin = confirm;
  }
  return errors;
}

export function mapServerError(message: string): AdminField | null {
  const lower = message.toLowerCase();
  if (lower.includes("pin")) return "pin";
  if (lower.includes("playlist")) return "playlistUrl";
  if (lower.includes("client id") || lower.includes("client")) return "spotifyClientId";
  if (lower.includes("market")) return "market";
  if (lower.includes("country")) return "country";
  return null;
}
