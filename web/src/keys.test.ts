import { describe, expect, it } from "vitest";

import { initialState, reduce } from "./keys";

describe("keys", () => {
  it("scrolls with j and k", () => {
    const start = initialState();
    expect(reduce(start, { key: "j" }).command).toEqual({ type: "line", dir: 1 });
    expect(reduce(start, { key: "k" }).command).toEqual({ type: "line", dir: -1 });
    expect(reduce(start, { key: "ArrowDown" }).command).toEqual({ type: "line", dir: 1 });
  });

  it("treats gg as top and G as bottom", () => {
    const start = initialState();
    const pending = reduce(start, { key: "g" });
    expect(pending.command).toEqual({ type: "none" });
    expect(pending.state.pendingG).toBe(true);
    expect(reduce(pending.state, { key: "g" }).command).toEqual({ type: "top" });
    expect(reduce(start, { key: "G" }).command).toEqual({ type: "bottom" });
    const interrupted = reduce(pending.state, { key: "j" });
    expect(interrupted.command).toEqual({ type: "line", dir: 1 });
    expect(interrupted.state.pendingG).toBe(false);
  });

  it("opens search and steps matches", () => {
    const start = initialState();
    const opened = reduce(start, { key: "/" });
    expect(opened.command).toEqual({ type: "searchStart" });
    expect(opened.state.mode).toBe("search");
    expect(reduce(opened.state, { key: "n" }).command).toEqual({ type: "none" });
    const committed = reduce(opened.state, { key: "Enter" });
    expect(committed.command).toEqual({ type: "searchCommit" });
    expect(committed.state.mode).toBe("read");
    expect(reduce(committed.state, { key: "n" }).command).toEqual({ type: "find", dir: 1 });
    expect(reduce(committed.state, { key: "N" }).command).toEqual({ type: "find", dir: -1 });
    const cancelled = reduce(opened.state, { key: "Escape" });
    expect(cancelled.command).toEqual({ type: "searchCancel" });
  });
});
