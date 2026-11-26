import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

describe("app command ACL", () => {
  it("grants every command registered in build.rs", () => {
    const root = process.cwd();
    const build = readFileSync(join(root, "src-tauri/build.rs"), "utf8");
    const commands = [...build.matchAll(/"([a-z_]+)"/g)]
      .map((match) => match[1])
      .filter((value): value is string => Boolean(value))
      .sort();

    const capability = JSON.parse(
      readFileSync(join(root, "src-tauri/capabilities/kiosk.json"), "utf8"),
    ) as { permissions: string[] };
    const allowed = capability.permissions
      .filter((permission) => permission.startsWith("allow-"))
      .map((permission) => permission.slice("allow-".length).replaceAll("-", "_"))
      .sort();

    expect(commands.length).toBeGreaterThan(10);
    expect(allowed).toEqual(commands);
  });
});
