import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { SearchResponse } from "../lib/types";
import { useKiosk } from "./useKiosk";

type Resolver = (value: SearchResponse) => void;

const track = (title: string, id: string) => ({
  id,
  uri: `spotify:track:${id}`,
  title,
  artists: ["Artist"],
  album: "Album",
  albumId: `album-${id}`,
  image640: null,
  image64: null,
});

describe("useKiosk search sequencing", () => {
  afterEach(() => {
    cleanup();
    clearMocks();
  });

  it("ignores a stale search response that resolves after a newer query", async () => {
    const resolvers = new Map<string, Resolver>();
    mockIPC((cmd, payload) => {
      if (cmd === "get_public_config") {
        return { babyName: "Ada", welcome: "A song for Ada", defaultOrientation: "portrait" };
      }
      if (cmd === "search_tracks") {
        const query = (payload as { query?: string } | undefined)?.query ?? "";
        if (query !== "ha" && query !== "halo") return { tracks: [], degraded: false };
        return new Promise<SearchResponse>((resolve) => {
          resolvers.set(query, resolve);
        });
      }
      return null;
    }, { shouldMockEvents: true });

    const { result } = renderHook(() => useKiosk());

    act(() => result.current.dispatch({ type: "SET_NAME", name: "Ada" }));
    act(() => result.current.dispatch({ type: "NAME_SUBMIT" }));
    act(() => result.current.dispatch({ type: "SET_QUERY", query: "ha" }));

    await waitFor(() => expect(resolvers.get("ha")).toBeDefined(), { timeout: 2000 });

    act(() => result.current.dispatch({ type: "SET_QUERY", query: "halo" }));
    await waitFor(() => expect(resolvers.get("halo")).toBeDefined(), { timeout: 2000 });

    await act(async () => {
      resolvers.get("halo")?.({ tracks: [track("New", "n1")], degraded: false });
    });
    await waitFor(() => {
      expect(result.current.state.screen).toBe("search");
      if (result.current.state.screen === "search") {
        expect(result.current.state.results[0]?.title).toBe("New");
      }
    });

    await act(async () => {
      resolvers.get("ha")?.({ tracks: [track("Old", "o1")], degraded: false });
    });
    await waitFor(() => {
      if (result.current.state.screen === "search") {
        expect(result.current.state.results[0]?.title).toBe("New");
      }
    });
  });
});
