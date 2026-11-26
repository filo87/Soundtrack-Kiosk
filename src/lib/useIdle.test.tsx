import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useIdle } from "./useIdle";

describe("useIdle", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("fires after the timeout and resets on activity", () => {
    vi.useFakeTimers();
    const onIdle = vi.fn();
    renderHook(({ key }) => useIdle(1000, onIdle, key), {
      initialProps: { key: "name" },
    });

    act(() => {
      vi.advanceTimersByTime(999);
    });
    expect(onIdle).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(onIdle).toHaveBeenCalledTimes(1);

    act(() => {
      vi.advanceTimersByTime(900);
      window.dispatchEvent(new KeyboardEvent("keydown"));
      vi.advanceTimersByTime(999);
    });
    expect(onIdle).toHaveBeenCalledTimes(1);

    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(onIdle).toHaveBeenCalledTimes(2);
  });

  it("restarts the timer when the reset key changes", () => {
    vi.useFakeTimers();
    const onIdle = vi.fn();
    const { rerender } = renderHook(({ key }) => useIdle(1000, onIdle, key), {
      initialProps: { key: "name" },
    });

    act(() => {
      vi.advanceTimersByTime(800);
    });
    rerender({ key: "search" });
    act(() => {
      vi.advanceTimersByTime(999);
    });
    expect(onIdle).not.toHaveBeenCalled();
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(onIdle).toHaveBeenCalledTimes(1);
  });
});
