import { useCallback, useEffect, useRef, useState } from "react";
import { Copy, ListMusic, LogIn, LogOut, Plus, RefreshCw } from "lucide-react";
import { ipc } from "../../lib/ipc";
import type { SpotifyPlaylist } from "../../lib/types";
import { parsePlaylistId } from "../validation";
import type { AdminController } from "../useAdmin";
import { Button, Card, Field, StatusChip, TextInput } from "../ui";

function playlistUrl(id: string): string {
  return `https://open.spotify.com/playlist/${id}`;
}

async function copyText(text: string): Promise<boolean> {
  const clipboard = navigator.clipboard;
  const clipboardWorked =
    clipboard && typeof clipboard.writeText === "function"
      ? await clipboard.writeText(text).then(
          () => true,
          () => false,
        )
      : false;
  if (clipboardWorked) return true;
  const area = document.createElement("textarea");
  area.value = text;
  area.setAttribute("readonly", "");
  area.style.position = "fixed";
  area.style.opacity = "0";
  document.body.appendChild(area);
  area.select();
  const copied = (() => {
    try {
      return document.execCommand("copy");
    } catch {
      return false;
    }
  })();
  document.body.removeChild(area);
  return copied;
}

type PlaylistPhase = "loading" | "ready" | "failed";

export function SpotifySection({ admin }: { admin: AdminController }) {
  const { status, config } = admin;
  const [playlists, setPlaylists] = useState<SpotifyPlaylist[] | null>(null);
  const [phase, setPhase] = useState<PlaylistPhase>("loading");
  const [newName, setNewName] = useState("");
  const loadedRef = useRef(false);

  const connected = status?.health.auth ?? false;

  const loadPlaylists = useCallback(async () => {
    setPhase("loading");
    const result = await admin.run("list-playlists", () => ipc.listSpotifyPlaylists(), {
      reload: false,
    });
    if (result) {
      setPlaylists(result);
      setPhase("ready");
    } else {
      setPhase("failed");
    }
  }, [admin]);

  useEffect(() => {
    if (!connected) {
      loadedRef.current = false;
      setPlaylists(null);
      setPhase("loading");
      return;
    }
    if (loadedRef.current) return;
    loadedRef.current = true;
    void loadPlaylists();
  }, [connected, loadPlaylists]);

  if (!status || !config) return null;
  const playlistId = parsePlaylistId(config.playlistUrl);
  const playlistError = admin.fieldError("playlistUrl");

  const copyRedirect = async () => {
    const copied = await copyText(status.spotifyRedirectUri);
    admin.pushToast(
      copied ? "success" : "error",
      copied ? "Redirect URI copied." : "Could not copy. Select the URI and copy it manually.",
    );
  };

  const selectPlaylist = (playlist: SpotifyPlaylist) => {
    admin.setField("playlistUrl", playlistUrl(playlist.id));
  };

  const createPlaylist = async () => {
    const name = newName.trim();
    if (!name) return;
    await admin.run("create-playlist", () => ipc.createSpotifyPlaylist(name), {
      reload: false,
      success: (playlist) => `Created “${playlist.name}”. Save to apply.`,
      onSuccess: (playlist) => {
        admin.setField("playlistUrl", playlistUrl(playlist.id));
        setNewName("");
        void loadPlaylists();
      },
    });
  };

  return (
    <div className="space-y-5">
      <Card
        title="Connection"
        description="Search, covers and playlist adds need a Spotify login. Login opens the system browser."
        actions={
          <StatusChip tone={connected ? "ok" : "attention"}>
            {connected ? "Connected" : "Login needed"}
          </StatusChip>
        }
      >
        <div className="flex flex-wrap gap-2">
          <Button
            variant="primary"
            onClick={() =>
              void admin.run("auth-start", () => ipc.authStart(), {
                success: "Spotify login complete.",
                onSuccess: () => {
                  loadedRef.current = false;
                },
              })
            }
            pending={admin.isPending("auth-start")}
          >
            <LogIn aria-hidden className="h-4 w-4" />
            Login / relogin
          </Button>
          <Button
            onClick={() =>
              void admin.run("auth-logout", () => ipc.authLogout(), {
                success: "Logged out.",
              })
            }
            pending={admin.isPending("auth-logout")}
          >
            <LogOut aria-hidden className="h-4 w-4" />
            Log out
          </Button>
        </div>
        <p className="mt-3 text-xs text-muted">
          The playlist owner must match the logged-in account. Ownership is checked when settings
          are saved.
        </p>
      </Card>

      <Card
        title="Playlist"
        description="Pick one of your Spotify playlists or create a new one for this event."
        actions={
          connected ? (
            <Button
              onClick={() => void loadPlaylists()}
              pending={admin.isPending("list-playlists")}
            >
              <RefreshCw aria-hidden className="h-4 w-4" />
              Refresh
            </Button>
          ) : null
        }
      >
        {!connected ? (
          <p className="text-sm text-muted">
            Log in to Spotify to list or create playlists for this event.
          </p>
        ) : (
          <div className="space-y-4">
            {phase === "loading" ? (
              <p className="text-sm text-muted">Loading your playlists…</p>
            ) : phase === "failed" ? (
              <p className="text-sm text-muted">
                Could not load your playlists. Press Refresh to try again.
              </p>
            ) : playlists && playlists.length > 0 ? (
              <ul className="grid gap-2 sm:grid-cols-2">
                {playlists.map((playlist) => {
                  const selected = playlist.id === playlistId;
                  return (
                    <li key={playlist.id}>
                      <button
                        type="button"
                        aria-pressed={selected}
                        onClick={() => selectPlaylist(playlist)}
                        className={`flex w-full items-center gap-3 rounded-xl border px-3 py-2.5 text-left transition ${
                          selected
                            ? "border-cerulean bg-cerulean/15 text-ink shadow-[inset_0_-2px_0_0_var(--color-cerulean)]"
                            : "border-steel/60 bg-sand/25 text-muted hover:bg-wheat/60 hover:text-ink"
                        }`}
                      >
                        <ListMusic aria-hidden className="h-4 w-4 shrink-0" />
                        <span className="min-w-0 flex-1">
                          <span className="block truncate text-sm font-medium text-ink">
                            {playlist.name}
                          </span>
                          <span className="block truncate text-xs text-muted">
                            {playlist.trackCount}{" "}
                            {playlist.trackCount === 1 ? "track" : "tracks"} · {playlist.owner}
                          </span>
                        </span>
                        {selected ? (
                          <span className="shrink-0 text-xs font-medium text-cerulean">
                            Selected
                          </span>
                        ) : null}
                      </button>
                    </li>
                  );
                })}
              </ul>
            ) : (
              <p className="text-sm text-muted">
                No playlists owned by this account yet. Create one below.
              </p>
            )}
            <div className="flex flex-wrap items-end gap-3">
              <label className="min-w-[14rem] flex-1 text-sm">
                <span className="mb-1.5 block font-medium text-ink">New playlist name</span>
                <TextInput
                  value={newName}
                  onChange={(event) => setNewName(event.target.value)}
                  placeholder="e.g. Ada's Baby Shower"
                  aria-label="New playlist name"
                  autoComplete="off"
                  maxLength={100}
                />
              </label>
              <Button
                variant="primary"
                disabled={!newName.trim()}
                pending={admin.isPending("create-playlist")}
                onClick={() => void createPlaylist()}
              >
                <Plus aria-hidden className="h-4 w-4" />
                Create playlist
              </Button>
            </div>
          </div>
        )}

        <p className="mt-4 text-xs text-muted">
          {playlistId ? `Current playlist id: ${playlistId}.` : "No playlist selected yet."}{" "}
          Choosing or creating a playlist selects it; press Save to apply.
        </p>
        {playlistError ? (
          <p className="mt-2 flex items-start gap-1.5 rounded-lg bg-salmon/25 px-2.5 py-1.5 text-xs font-medium text-ink">
            <span aria-hidden className="mt-1 h-1.5 w-1.5 shrink-0 rounded-full bg-salmon" />
            {playlistError}
          </p>
        ) : null}
      </Card>

      <Card title="Account and region">
        <div className="grid gap-4 md:grid-cols-2">
          <Field
            label="Spotify client id"
            hint="Spotify Developer Dashboard → your app → Client ID."
            error={admin.fieldError("spotifyClientId")}
          >
            <TextInput
              value={config.spotifyClientId}
              onChange={(event) => admin.setField("spotifyClientId", event.target.value)}
              onBlur={() => admin.markTouched("spotifyClientId")}
              autoComplete="off"
              spellCheck={false}
              aria-label="Spotify client id"
              aria-invalid={Boolean(admin.fieldError("spotifyClientId"))}
            />
          </Field>
          <Field
            label="Market"
            hint="Two-letter Spotify market, e.g. US."
            error={admin.fieldError("market")}
          >
            <TextInput
              value={config.market}
              onChange={(event) => admin.setField("market", event.target.value)}
              onBlur={() => admin.markTouched("market")}
              autoComplete="off"
              aria-label="Market"
              aria-invalid={Boolean(admin.fieldError("market"))}
            />
          </Field>
          <Field
            label="iTunes country"
            hint="Two-letter iTunes storefront, e.g. US."
            error={admin.fieldError("country")}
          >
            <TextInput
              value={config.country}
              onChange={(event) => admin.setField("country", event.target.value)}
              onBlur={() => admin.markTouched("country")}
              autoComplete="off"
              aria-label="iTunes country"
              aria-invalid={Boolean(admin.fieldError("country"))}
            />
          </Field>
        </div>
      </Card>

      <Card
        title="Redirect URI"
        description="Also listed in the Spotify Developer Dashboard setup."
      >
        <div className="flex flex-wrap items-end gap-3">
          <label className="min-w-[16rem] flex-1 text-sm">
            <span className="mb-1.5 block font-medium text-ink">Redirect URI</span>
            <TextInput
              readOnly
              value={status.spotifyRedirectUri}
              aria-label="Redirect URI"
              onFocus={(event) => event.currentTarget.select()}
              className="font-mono text-sm"
            />
          </label>
          <Button onClick={() => void copyRedirect()}>
            <Copy aria-hidden className="h-4 w-4" />
            Copy
          </Button>
        </div>
        <p className="mt-3 text-xs text-muted">
          Add this exact URI to the Spotify app. It must match character for character; the callback
          port is fixed at 43821.
        </p>
      </Card>
    </div>
  );
}
