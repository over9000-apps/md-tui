import { describe, expect, it } from "vitest";

import { collectHits, foldsAsciiCase, setCurrentHit } from "./search";

describe("search", () => {
  it("folds only an all-lowercase query", () => {
    expect(foldsAsciiCase("table")).toBe(true);
    expect(foldsAsciiCase("Table")).toBe(false);
    expect(foldsAsciiCase("")).toBe(false);
  });

  it("wraps matches and moves the current hit", () => {
    const root = document.createElement("article");
    root.innerHTML = "<p>Scroll the table, then another Table.</p>";
    const hits = collectHits(root, "table");
    expect(hits).toHaveLength(2);
    expect(hits[0].textContent).toBe("table");
    expect(hits[1].textContent).toBe("Table");
    expect(setCurrentHit(hits, 0)).toBe(0);
    expect(hits[0].classList.contains("current")).toBe(true);
    expect(setCurrentHit(hits, 1)).toBe(1);
    expect(setCurrentHit(hits, 2)).toBe(0);
    const again = collectHits(root, "Table");
    expect(again).toHaveLength(1);
    expect(again[0].textContent).toBe("Table");
  });
});
