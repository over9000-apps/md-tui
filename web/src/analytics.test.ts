import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

describe("analytics", () => {
  it("includes the GA4 tag and canonical host in the page shell", () => {
    const html = readFileSync(join(process.cwd(), "index.html"), "utf8");
    expect(html).toContain("G-FG52JBJRPS");
    expect(html).toContain("https://www.googletagmanager.com/gtag/js?id=G-FG52JBJRPS");
    expect(html).toContain('rel="canonical" href="https://md-tui.over9000.cc/"');
  });
});
