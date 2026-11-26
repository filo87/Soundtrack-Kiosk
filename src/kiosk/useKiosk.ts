import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { ipc } from "../lib/ipc";
import type { Orientation, PrintLayout, PublicConfig, Track } from "../lib/types";
import { useIdle } from "../lib/useIdle";
import {
  initialState,
  reducer,
  timeoutFor,
  type KioskAction,
  type KioskState,
} from "./machine";

function newId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
}

export interface KioskController {
  state: KioskState;
  config: PublicConfig | null;
  dispatch: React.Dispatch<KioskAction>;
  pickTrack: (track: Track) => void;
  pickManual: (title: string, artist: string) => void;
  commit: (orientation: Orientation, layout: PrintLayout) => Promise<void>;
  cancelChoose: () => void;
}

export function useKiosk(): KioskController {
  const [state, dispatch] = useReducer(reducer, initialState);
  const [config, setConfig] = useState<PublicConfig | null>(null);
  const stateRef = useRef(state);
  stateRef.current = state;
  const configRef = useRef(config);
  configRef.current = config;
  const searchSeq = useRef(0);
  const commitInFlight = useRef(false);
  const commitRef = useRef<(orientation: Orientation, layout: PrintLayout) => Promise<void>>(
    async () => {},
  );

  useEffect(() => {
    let cancelled = false;
    ipc
      .getPublicConfig()
      .then((value) => {
        if (!cancelled) setConfig(value);
      })
      .catch(() => {
        // The kiosk falls back to default copy when the shell is unavailable.
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const searchQuery = state.screen === "search" ? state.query : "";

  useEffect(() => {
    const seq = ++searchSeq.current;
    const query = searchQuery.trim();
    if (query.length < 2) return;

    const timer = setTimeout(async () => {
      try {
        const response = await ipc.searchTracks(query);
        if (searchSeq.current === seq) {
          dispatch({
            type: "SEARCH_RESULT",
            tracks: response.tracks,
            degraded: response.degraded,
          });
        }
      } catch {
        if (searchSeq.current === seq) {
          dispatch({ type: "SEARCH_RESULT", tracks: [], degraded: true });
        }
      }
    }, 250);

    return () => clearTimeout(timer);
  }, [searchQuery]);

  const highlightedIndex = state.screen === "search" ? state.highlighted : -1;
  const highlightedId =
    state.screen === "search" ? (state.results[highlightedIndex]?.id ?? null) : null;

  useEffect(() => {
    if (!highlightedId) return;
    const snapshot = stateRef.current;
    if (snapshot.screen !== "search") return;
    const track = snapshot.results[snapshot.highlighted];
    if (track) {
      void ipc.prefetchCover(track).catch(() => undefined);
    }
  }, [highlightedId]);

  const commitId = state.screen === "choose" ? state.commitId : null;

  useEffect(() => {
    if (!commitId) return;
    const snapshot = stateRef.current;
    if (snapshot.screen !== "choose" || snapshot.prep) return;
    let cancelled = false;
    ipc
      .prepareSelection(snapshot.selection)
      .then((prep) => {
        if (cancelled) {
          void ipc.discardPrepared(prep.prepId).catch(() => undefined);
          return;
        }
        dispatch({ type: "PREPARED", prep });
      })
      .catch(() => {
        if (!cancelled) dispatch({ type: "BACK" });
      });
    return () => {
      cancelled = true;
    };
  }, [commitId]);

  const commit = useCallback(async (orientation: Orientation, layout: PrintLayout) => {
    const snapshot = stateRef.current;
    if (snapshot.screen !== "choose" || !snapshot.prep || snapshot.committing) {
      return;
    }
    if (commitInFlight.current) return;
    commitInFlight.current = true;
    dispatch({ type: "COMMIT_START", orientation, layout });
    try {
      const result = await ipc.commitSelection({
        id: snapshot.commitId,
        prepId: snapshot.prep.prepId,
        guestName: snapshot.guestName,
        orientation,
        layout,
      });
      dispatch({ type: "COMMITTED", result });
    } catch {
      dispatch({ type: "COMMIT_FAILED" });
    } finally {
      commitInFlight.current = false;
    }
  }, []);

  commitRef.current = commit;

  const pickTrack = useCallback((track: Track) => {
    dispatch({
      type: "PICK_TRACK",
      track,
      commitId: newId(),
      orientation: configRef.current?.defaultOrientation ?? "portrait",
    });
  }, []);

  const cancelChoose = useCallback(() => {
    const snapshot = stateRef.current;
    if (snapshot.screen === "choose" && snapshot.prep && !snapshot.committing) {
      void ipc.discardPrepared(snapshot.prep.prepId).catch(() => undefined);
    }
    dispatch({ type: "BACK" });
  }, []);

  const pickManual = useCallback((title: string, artist: string) => {
    dispatch({
      type: "PICK_MANUAL",
      title,
      artist,
      commitId: newId(),
      orientation: configRef.current?.defaultOrientation ?? "portrait",
    });
  }, []);

  useIdle(
    timeoutFor(state),
    () => {
      const snapshot = stateRef.current;
      if (snapshot.screen === "name") {
        dispatch({ type: "SET_NAME", name: "" });
      } else if (snapshot.screen === "choose") {
        void commitRef.current(snapshot.orientation, snapshot.layout);
      } else {
        dispatch({ type: "RESET" });
      }
    },
    state.screen,
  );

  return { state, config, dispatch, pickTrack, pickManual, commit, cancelChoose };
}
