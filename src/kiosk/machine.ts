import type {
  CommitResult,
  Orientation,
  PrepareResult,
  PrintLayout,
  SelectionInput,
  Track,
} from "../lib/types";

export type KioskState =
  | { screen: "name"; name: string }
  | {
      screen: "search";
      guestName: string;
      query: string;
      results: Track[];
      degraded: boolean;
      highlighted: number;
    }
  | {
      screen: "choose";
      guestName: string;
      selection: SelectionInput;
      prep: PrepareResult | null;
      orientation: Orientation;
      layout: PrintLayout;
      commitId: string;
      committing: boolean;
    }
  | {
      screen: "done";
      guestName: string;
      orientation: Orientation;
      printFile: string | null;
      printsAhead: number;
    };

export type KioskAction =
  | { type: "SET_NAME"; name: string }
  | { type: "NAME_SUBMIT" }
  | { type: "SET_QUERY"; query: string }
  | { type: "SEARCH_RESULT"; tracks: Track[]; degraded: boolean }
  | { type: "HIGHLIGHT"; index: number }
  | { type: "MOVE_HIGHLIGHT"; delta: number }
  | {
      type: "PICK_TRACK";
      track: Track;
      commitId: string;
      orientation: Orientation;
    }
  | {
      type: "PICK_MANUAL";
      title: string;
      artist: string;
      commitId: string;
      orientation: Orientation;
    }
  | { type: "PREPARED"; prep: PrepareResult }
  | { type: "SET_ORIENTATION"; orientation: Orientation }
  | { type: "SET_LAYOUT"; layout: PrintLayout }
  | { type: "COMMIT_START"; orientation: Orientation; layout: PrintLayout }
  | { type: "COMMIT_FAILED" }
  | { type: "COMMITTED"; result: CommitResult }
  | { type: "BACK" }
  | { type: "RESET" };

export const initialState: KioskState = { screen: "name", name: "" };

export const SCREEN_TIMEOUTS = {
  name: 60_000,
  search: 60_000,
  choose: 30_000,
  done: 12_000,
} as const;

export function timeoutFor(state: KioskState): number {
  return SCREEN_TIMEOUTS[state.screen];
}

export function reducer(state: KioskState, action: KioskAction): KioskState {
  switch (action.type) {
    case "SET_NAME":
      return state.screen === "name" ? { ...state, name: action.name } : state;

    case "NAME_SUBMIT": {
      if (state.screen !== "name") return state;
      const guestName = state.name.trim();
      if (guestName.length < 1 || guestName.length > 40) return state;
      return {
        screen: "search",
        guestName,
        query: "",
        results: [],
        degraded: false,
        highlighted: 0,
      };
    }

    case "SET_QUERY": {
      if (state.screen !== "search") return state;
      if (state.query === action.query) return state;
      return { ...state, query: action.query, results: [], highlighted: 0 };
    }

    case "SEARCH_RESULT": {
      if (state.screen !== "search") return state;
      const max = Math.max(action.tracks.length - 1, 0);
      return {
        ...state,
        results: action.tracks,
        degraded: action.degraded,
        highlighted: Math.min(state.highlighted, max),
      };
    }

    case "HIGHLIGHT": {
      if (state.screen !== "search" || state.results.length === 0) return state;
      const index = Math.min(Math.max(action.index, 0), state.results.length - 1);
      return { ...state, highlighted: index };
    }

    case "MOVE_HIGHLIGHT": {
      if (state.screen !== "search" || state.results.length === 0) return state;
      const next = Math.min(
        Math.max(state.highlighted + action.delta, 0),
        state.results.length - 1,
      );
      return { ...state, highlighted: next };
    }

    case "PICK_TRACK": {
      if (state.screen !== "search") return state;
      return {
        screen: "choose",
        guestName: state.guestName,
        selection: { kind: "track", track: action.track },
        prep: null,
        orientation: action.orientation,
        layout: "cover",
        commitId: action.commitId,
        committing: false,
      };
    }

    case "PICK_MANUAL": {
      if (state.screen !== "search") return state;
      const title = action.title.trim();
      const artist = action.artist.trim();
      if (!title || !artist) return state;
      return {
        screen: "choose",
        guestName: state.guestName,
        selection: { kind: "manual", title, artist },
        prep: null,
        orientation: action.orientation,
        layout: "cover",
        commitId: action.commitId,
        committing: false,
      };
    }

    case "PREPARED":
      if (state.screen !== "choose" || state.prep) return state;
      return { ...state, prep: action.prep };

    case "SET_ORIENTATION":
      if (state.screen !== "choose" || state.committing) return state;
      return { ...state, orientation: action.orientation };

    case "SET_LAYOUT":
      if (state.screen !== "choose" || state.committing) return state;
      return { ...state, layout: action.layout };

    case "COMMIT_START":
      if (state.screen !== "choose") return state;
      return {
        ...state,
        orientation: action.orientation,
        layout: action.layout,
        committing: true,
      };

    case "COMMIT_FAILED":
      if (state.screen !== "choose") return state;
      return { ...state, committing: false };

    case "COMMITTED": {
      if (state.screen !== "choose") return state;
      return {
        screen: "done",
        guestName: state.guestName,
        orientation: state.orientation,
        printFile: action.result.printFile,
        printsAhead: action.result.printsAhead,
      };
    }

    case "BACK": {
      if (state.screen === "search") {
        if (state.query.length > 0) {
          return { ...state, query: "", results: [], degraded: false, highlighted: 0 };
        }
        return { screen: "name", name: state.guestName };
      }
      if (state.screen === "choose") {
        if (state.committing) return state;
        return {
          screen: "search",
          guestName: state.guestName,
          query: "",
          results: [],
          degraded: false,
          highlighted: 0,
        };
      }
      return state;
    }

    case "RESET":
      return initialState;
  }
}
