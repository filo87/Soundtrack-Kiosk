import { afterEach, describe, expect, it } from "vitest";
import { KioskError, toKioskError } from "./ipc";

describe("toKioskError", () => {
  it("keeps known codes and messages", () => {
    const error = toKioskError({ code: "QUOTA", message: "quota" });
    expect(error).toBeInstanceOf(KioskError);
    expect(error.code).toBe("QUOTA");
    expect(error.message).toBe("quota");
  });

  it("maps unknown shapes to INTERNAL", () => {
    expect(toKioskError({ code: "NOPE", message: "x" }).code).toBe("INTERNAL");
    expect(toKioskError("boom")).toMatchObject({ code: "INTERNAL", message: "boom" });
    expect(toKioskError(null).code).toBe("INTERNAL");
  });

  afterEach(() => {
    // no shared state
  });
});
