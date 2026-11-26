import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { clearMocks, mockConvertFileSrc, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";

const track = {
  id: "t1",
  uri: "spotify:track:t1",
  title: "Halo",
  artists: ["Beyoncé"],
  album: "I Am... Sasha Fierce",
  albumId: "a1",
  image640: null,
  image64: null,
};

const prep = {
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

let degraded = false;
let lastCommit: { orientation?: string; layout?: string } | null = null;

function install() {
  mockConvertFileSrc("macos");
  mockIPC((cmd, payload) => {
    switch (cmd) {
      case "get_public_config":
        return { babyName: "Ada", welcome: "A song for Ada", defaultOrientation: "portrait" };
      case "search_tracks":
        return degraded ? { tracks: [], degraded: true } : { tracks: [track], degraded: false };
      case "prepare_selection":
        return prep;
      case "commit_selection":
        lastCommit = payload as { orientation?: string; layout?: string };
        return { printsAhead: 0, printQueued: true, printFile: "/tmp/print.png" };
      case "plugin:event|listen":
        return 1;
      case "plugin:event|unlisten":
        return null;
      default:
        return null;
    }
  }, { shouldMockEvents: true });
}

async function enterName(user: ReturnType<typeof userEvent.setup>, name: string) {
  const input = await screen.findByLabelText("Your name");
  await user.type(input, name);
  await user.keyboard("{Enter}");
}

describe("guest flow", () => {
  afterEach(() => {
    cleanup();
    clearMocks();
  });

  it("runs name, search, choose, print and done", async () => {
    degraded = false;
    install();
    const user = userEvent.setup();
    render(<App />);

    await enterName(user, "Ada");
    const search = await screen.findByLabelText("Search songs");
    await user.type(search, "Halo");

    const option = await screen.findByRole("option", { name: /Halo/ }, { timeout: 3000 });
    expect(option).toBeInTheDocument();
    await user.keyboard("{Enter}");

    await screen.findByAltText("Portrait · Cover preview", {}, { timeout: 3000 });
    await user.keyboard("{Enter}");

    expect(await screen.findByText(/Thank you, Ada!/)).toBeInTheDocument();
    expect(screen.getByText(/on its way/)).toBeInTheDocument();
    expect(lastCommit).toMatchObject({ orientation: "portrait", layout: "cover" });
  });

  it("falls back to manual entry when search is degraded", async () => {
    degraded = true;
    install();
    const user = userEvent.setup();
    render(<App />);

    await enterName(user, "Grace");
    const search = await screen.findByLabelText("Search songs");
    await user.type(search, "Yellow");

    const song = await screen.findByLabelText("Song");
    await user.type(song, "Yellow");
    const artist = screen.getByLabelText("Artist");
    await user.type(artist, "Coldplay");
    await user.keyboard("{Enter}");

    await screen.findByAltText("Portrait · Cover preview", {}, { timeout: 3000 });
    await user.keyboard("{Enter}");
    expect(await screen.findByText(/Thank you, Grace!/)).toBeInTheDocument();
  });

  it("offers four layouts and commits a landscape fit via keyboard", async () => {
    degraded = false;
    lastCommit = null;
    install();
    const user = userEvent.setup();
    render(<App />);

    await enterName(user, "Ada");
    const search = await screen.findByLabelText("Search songs");
    await user.type(search, "Halo");
    await screen.findByRole("option", { name: /Halo/ }, { timeout: 3000 });
    await user.keyboard("{Enter}");
    await screen.findByAltText("Landscape · Fit preview", {}, { timeout: 3000 });

    expect(screen.getByLabelText("Print Portrait · Cover")).toBeInTheDocument();
    expect(screen.getByLabelText("Print Portrait · Fit")).toBeInTheDocument();
    expect(screen.getByLabelText("Print Landscape · Cover")).toBeInTheDocument();
    expect(screen.getByLabelText("Print Landscape · Fit")).toBeInTheDocument();

    await user.keyboard("{ArrowRight}{ArrowDown}");
    await user.keyboard("{Enter}");
    expect(await screen.findByText(/Thank you, Ada!/)).toBeInTheDocument();
    expect(lastCommit).toMatchObject({ orientation: "landscape", layout: "fit" });
  });

  it("does not commit when Escape is pressed on the choose screen", async () => {
    degraded = false;
    install();
    const user = userEvent.setup();
    render(<App />);

    await enterName(user, "Ada");
    const search = await screen.findByLabelText("Search songs");
    await user.type(search, "Halo");
    await screen.findByRole("option", { name: /Halo/ }, { timeout: 3000 });
    await user.keyboard("{Enter}");
    await screen.findByAltText("Portrait · Cover preview", {}, { timeout: 3000 });

    await user.keyboard("{Escape}");
    expect(await screen.findByLabelText("Search songs")).toBeInTheDocument();
    expect(screen.queryByText(/Thank you/)).not.toBeInTheDocument();
  });
});
