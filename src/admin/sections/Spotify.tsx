import { Copy, LogIn, LogOut } from "lucide-react";
import { ipc } from "../../lib/ipc";
import { parsePlaylistId } from "../validation";
import type { AdminController } from "../useAdmin";
import { Button, Card, Field, StatusChip, TextInput } from "../ui";

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

export function SpotifySection({ admin }: { admin: AdminController }) {
  const { status, config } = admin;
  if (!status || !config) return null;
  const connected = status.health.auth;
  const playlistId = parsePlaylistId(config.playlistUrl);

  const copyRedirect = async () => {
    const copied = await copyText(status.spotifyRedirectUri);
    admin.pushToast(
      copied ? "success" : "error",
      copied ? "Redirect URI copied." : "Could not copy. Select the URI and copy it manually.",
    );
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

      <Card title="Playlist and account">
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
            label="Playlist URL"
            hint={
              config.playlistUrl.trim()
                ? playlistId
                  ? `Playlist id: ${playlistId}`
                  : "Paste a full Spotify playlist URL, spotify:playlist:…, or the id."
                : "Paste a Spotify playlist URL, spotify:playlist:…, or the playlist id."
            }
            error={admin.fieldError("playlistUrl")}
          >
            <TextInput
              value={config.playlistUrl}
              onChange={(event) => admin.setField("playlistUrl", event.target.value)}
              onBlur={() => admin.markTouched("playlistUrl")}
              autoComplete="off"
              spellCheck={false}
              aria-label="Playlist URL"
              aria-invalid={Boolean(admin.fieldError("playlistUrl"))}
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
