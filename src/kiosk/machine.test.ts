import { describe, expect, it } from "vitest";
import type { PrepareResult, Track } from "../lib/types";
import { initialState, reducer, timeoutFor, type KioskState } from "./machine";

const track: Track = {
  id: "t1",
  uri: "spotify:track:t1",
  title: "Halo",
  artists: ["Beyoncé"],
  album: "I Am... Sasha Fierce",
  albumId: "a1",
  image640: "https://img/640.jpg",
  image64: "https://img/64.jpg",
};

const prep: PrepareResult = {
  prepId: "p1",
  previews: {
    portraitCover: "/tmp/pc.png",
    portraitFit: "/tmp/pf.png",
    landscapeCover: "/tmp/lc.png",
    landscapeFit: "/tmp/lf.png",
  },
  coverSource: "itunes",
  coverPx: 3000,
};

function searchState(): Extract<KioskState, { screen: "search" }> {
  return {
    screen: "search",
    guestName: "Ada",
    query: "",
    results: [],
    degraded: false,
    highlighted: 0,
  };
}

describe("kiosk reducer", () => {
  it("edits the name and only continues with a valid trimmed name", () => {
    const named = reducer(initialState, { type: "SET_NAME", name: "  Ada  " });
    const entered = reducer(named, { type: "NAME_SUBMIT" });
    expect(entered).toMatchObject({ screen: "search", guestName: "Ada", query: "" });
    expect(reducer({ screen: "name", name: "   " }, { type: "NAME_SUBMIT" })).toEqual({
      screen: "name",
      name: "   ",
    });
    expect(
      reducer({ screen: "name", name: "a".repeat(41) }, { type: "NAME_SUBMIT" }),
    ).toMatchObject({ screen: "name" });
  });

  it("clears results when the query changes and clamps the highlight", () => {
    const initial = reducer(searchState(), { type: "SET_QUERY", query: "ha" });
    const loaded = reducer(initial, {
      type: "SEARCH_RESULT",
      tracks: [track, { ...track, id: "t2" }],
      degraded: false,
    });
    expect(loaded).toMatchObject({ highlighted: 0, results: [track, { ...track, id: "t2" }] });
    const moved = reducer(loaded, { type: "MOVE_HIGHLIGHT", delta: 5 });
    expect(moved).toMatchObject({ highlighted: 1 });
    expect(reducer(moved, { type: "MOVE_HIGHLIGHT", delta: -5 })).toMatchObject({
      highlighted: 0,
    });
    expect(reducer(loaded, { type: "SET_QUERY", query: "hal" })).toMatchObject({
      query: "hal",
      results: [],
      highlighted: 0,
    });
  });

  it("goes from a track pick to a prepared choose screen", () => {
    const search = { ...searchState(), results: [track] };
    const choose = reducer(search, {
      type: "PICK_TRACK",
      track,
      commitId: "c1",
      orientation: "landscape",
    });
    expect(choose).toMatchObject({
      screen: "choose",
      commitId: "c1",
      orientation: "landscape",
      layout: "cover",
      prep: null,
      committing: false,
      selection: { kind: "track", track },
    });
    if (choose.screen !== "choose") throw new Error("expected choose");
    expect(reducer(choose, { type: "PREPARED", prep })).toMatchObject({ prep });
    const prepared = reducer(choose, { type: "PREPARED", prep });
    expect(reducer(prepared, { type: "PREPARED", prep: { ...prep, prepId: "p2" } })).toBe(
      prepared,
    );
    const fitted = reducer(prepared, { type: "SET_LAYOUT", layout: "fit" });
    expect(fitted).toMatchObject({ layout: "fit", orientation: "landscape" });
    expect(reducer(fitted, { type: "SET_ORIENTATION", orientation: "portrait" })).toMatchObject({
      layout: "fit",
      orientation: "portrait",
    });
  });

  it("requires both fields for manual entry", () => {
    const search = { ...searchState(), degraded: true };
    expect(
      reducer(search, {
        type: "PICK_MANUAL",
        title: "Song",
        artist: "  ",
        commitId: "c1",
        orientation: "portrait",
      }),
    ).toBe(search);
    expect(
      reducer(search, {
        type: "PICK_MANUAL",
        title: " Song ",
        artist: " Artist ",
        commitId: "c1",
        orientation: "portrait",
      }),
    ).toMatchObject({
      screen: "choose",
      selection: { kind: "manual", title: "Song", artist: "Artist" },
    });
  });

  it("commits once and lands on the done screen", () => {
    const choose = reducer(
      { ...searchState(), results: [track] },
      { type: "PICK_TRACK", track, commitId: "c1", orientation: "portrait" },
    );
    if (choose.screen !== "choose") throw new Error("expected choose");
    const prepared = reducer(choose, { type: "PREPARED", prep });
    const starting = reducer(prepared, {
      type: "COMMIT_START",
      orientation: "landscape",
      layout: "fit",
    });
    expect(starting).toMatchObject({
      committing: true,
      orientation: "landscape",
      layout: "fit",
    });
    expect(reducer(starting, { type: "SET_ORIENTATION", orientation: "portrait" })).toMatchObject({
      orientation: "landscape",
    });
    expect(reducer(starting, { type: "SET_LAYOUT", layout: "cover" })).toMatchObject({
      layout: "fit",
    });
    const failed = reducer(starting, { type: "COMMIT_FAILED" });
    expect(failed).toMatchObject({ committing: false });
    const done = reducer(failed, {
      type: "COMMITTED",
      result: { printsAhead: 2, printQueued: true, printFile: "/tmp/print.png" },
    });
    expect(done).toEqual({
      screen: "done",
      guestName: "Ada",
      orientation: "landscape",
      printFile: "/tmp/print.png",
      printsAhead: 2,
    });
  });

  it("backs out of query, choose and done without saving", () => {
    const search = { ...searchState(), query: "halo" };
    expect(reducer(search, { type: "BACK" })).toMatchObject({ screen: "search", query: "" });
    expect(reducer({ ...searchState(), query: "" }, { type: "BACK" })).toEqual({
      screen: "name",
      name: "Ada",
    });
    const choose = reducer(
      { ...searchState(), results: [track] },
      { type: "PICK_TRACK", track, commitId: "c1", orientation: "portrait" },
    );
    expect(reducer(choose, { type: "BACK" })).toMatchObject({
      screen: "search",
      guestName: "Ada",
      query: "",
      results: [],
    });
    expect(reducer(choose, { type: "RESET" })).toEqual(initialState);
  });

  it("ignores back while a commit is in flight", () => {
    const choose = reducer(
      { ...searchState(), results: [track] },
      { type: "PICK_TRACK", track, commitId: "c1", orientation: "portrait" },
    );
    if (choose.screen !== "choose") throw new Error("expected choose");
    const prepared = reducer(choose, { type: "PREPARED", prep });
    const committing = reducer(prepared, {
      type: "COMMIT_START",
      orientation: "portrait",
      layout: "cover",
    });
    expect(reducer(committing, { type: "BACK" })).toBe(committing);
  });

  it("maps screens to their idle timeouts", () => {
    expect(timeoutFor({ screen: "name", name: "" })).toBe(60_000);
    expect(timeoutFor(searchState())).toBe(60_000);
    expect(
      timeoutFor({
        screen: "done",
        guestName: "Ada",
        orientation: "portrait",
        printFile: null,
        printsAhead: 0,
      }),
    ).toBe(12_000);
  });
});
