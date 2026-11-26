import { describe, expect, it } from "vitest";
import {
  parsePlaylistId,
  validateClientId,
  validateConfig,
  validatePin,
  validatePinConfirm,
  validatePlaylistUrl,
  validateRegionCode,
  validateWelcomeLine,
} from "./validation";

describe("validatePin", () => {
  it.each(["0123", "2468", "123456789012"])("accepts %s", (pin) => {
    expect(validatePin(pin)).toBeNull();
  });

  it.each(["", "123", "1234567890123", "12a4", " 1234", "0000"])("rejects %s", (pin) => {
    expect(validatePin(pin)).toMatch(/4-12 digits|other than 0000/);
  });
});

describe("validatePinConfirm", () => {
  it("accepts matching values", () => {
    expect(validatePinConfirm("2468", "2468")).toBeNull();
  });

  it("rejects a mismatch", () => {
    expect(validatePinConfirm("2468", "2469")).toBe("PINs do not match.");
  });
});

describe("parsePlaylistId", () => {
  const id = "37i9dQZF1DXcBWIGoYBM5M";

  it.each([
    `https://open.spotify.com/playlist/${id}`,
    `https://open.spotify.com/playlist/${id}?si=abc`,
    `http://open.spotify.com/intl-de/playlist/${id}`,
    `spotify:playlist:${id}`,
    `spotify:playlist:${id}?si=abc`,
    id,
  ])("parses %s", (input) => {
    expect(parsePlaylistId(input)).toBe(id);
  });

  it.each([
    "",
    "   ",
    "short",
    "https://open.spotify.com/album/37i9dQZF1DXcBWIGoYBM5M",
    "https://open.spotify.com/playlist/",
    "spotify:album:37i9dQZF1DXcBWIGoYBM5M",
    "not a playlist",
  ])("rejects %s", (input) => {
    expect(parsePlaylistId(input)).toBeNull();
  });
});

describe("validatePlaylistUrl", () => {
  it("allows an unconfigured playlist", () => {
    expect(validatePlaylistUrl("")).toBeNull();
  });

  it("rejects an invalid value", () => {
    expect(validatePlaylistUrl("https://example.com/nope")).toMatch(/not valid/);
  });
});

describe("validateClientId", () => {
  it("allows an unset client id", () => {
    expect(validateClientId("")).toBeNull();
  });

  it("accepts 32 hex characters", () => {
    expect(validateClientId("a1b2c3d4e5f60718293a4b5c6d7e8f90")).toBeNull();
  });

  it.each([
    "abc",
    "a1b2c3d4e5f60718293a4b5c6d7e8f9g",
    "a1b2c3d4e5f60718293a4b5c6d7e8f900",
    " a1b2c3d4e5f60718293a4b5c6d7e8f90",
  ])("rejects %s", (value) => {
    expect(validateClientId(value)).toMatch(/32 hex/);
  });
});

describe("validateRegionCode", () => {
  it.each(["US", "de", "Gb"])("accepts %s", (value) => {
    expect(validateRegionCode(value)).toBeNull();
  });

  it.each(["", "USA", "1A", "U", "US "])("rejects %s", (value) => {
    expect(validateRegionCode(value)).toMatch(/2-letter/);
  });
});

describe("validateWelcomeLine", () => {
  it("accepts a line at the limit", () => {
    expect(validateWelcomeLine("a".repeat(80))).toBeNull();
  });

  it("rejects a line over the limit", () => {
    expect(validateWelcomeLine("a".repeat(81))).toMatch(/80 characters/);
  });
});

describe("validateConfig", () => {
  const base = {
    babyName: "Ada",
    welcomeLine: "A song for {babyName}",
    playlistUrl: "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M",
    market: "US",
    country: "US",
    defaultOrientation: "portrait" as const,
    printer: null,
    pin: "2468",
    spotifyClientId: "a1b2c3d4e5f60718293a4b5c6d7e8f90",
  };

  it("returns no errors for a valid config", () => {
    expect(validateConfig(base)).toEqual({});
  });

  it("returns errors per field", () => {
    expect(
      validateConfig({
        ...base,
        playlistUrl: "bad",
        market: "USA",
        country: "",
        welcomeLine: "a".repeat(81),
        pin: "0000",
        spotifyClientId: "nope",
      }),
    ).toEqual({
      playlistUrl: expect.any(String),
      market: expect.any(String),
      country: expect.any(String),
      welcomeLine: expect.any(String),
      pin: expect.any(String),
      spotifyClientId: expect.any(String),
    });
  });

  it("checks the confirm value when provided", () => {
    expect(validateConfig(base, "9999")).toMatchObject({ pin: "PINs do not match." });
  });
});
