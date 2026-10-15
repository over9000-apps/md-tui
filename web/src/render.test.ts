import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { renderMarkdown } from "./render";
import { slugify } from "./slug";

const showcase = readFileSync(join(process.cwd(), "../fixtures/showcase.md"), "utf8");

describe("slugify", () => {
  it("matches the terminal reader", () => {
    expect(slugify("Why this exists")).toBe("why-this-exists");
    expect(slugify("What's new")).toBe("whats-new");
    expect(slugify("Tables")).toBe("tables");
  });
});

describe("renderMarkdown", () => {
  const { html, headings } = renderMarkdown(showcase);
  const root = () => {
    const div = document.createElement("div");
    div.innerHTML = html;
    return div;
  };

  it("typesets the showcase heading scale without raw hashes", () => {
    const doc = root();
    expect(doc.querySelector("h1")?.textContent).toBe("Showcase");
    expect(doc.querySelector("h2")?.textContent).toBe("Why this exists");
    expect(doc.querySelector("h3")?.textContent).toBe("What you can do");
    expect(html).not.toContain("# Showcase");
    expect(html).not.toContain("```");
    expect(headings.map((heading) => heading.slug)).toEqual(
      expect.arrayContaining(["showcase", "why-this-exists", "what-you-can-do", "tables"]),
    );
  });

  it("highlights fenced code and keeps the language label", () => {
    const doc = root();
    const langs = [...doc.querySelectorAll(".code-lang")].map((el) => el.textContent);
    expect(langs).toEqual(expect.arrayContaining(["bash", "rust", "python", "text"]));
    const rust = [...doc.querySelectorAll(".code-block")].find(
      (el) => el.querySelector(".code-lang")?.textContent === "rust",
    );
    expect(rust?.textContent).toContain("fn main");
    expect(rust?.querySelector(".hljs-keyword")?.textContent).toBe("fn");
    expect(rust?.textContent).toContain("hello");
  });

  it("renders tables, tasks, quotes, alerts, and links", () => {
    const doc = root();
    const headers = [...doc.querySelectorAll("th")].map((el) => el.textContent?.trim());
    expect(headers.slice(0, 3)).toEqual(["Feature", "Keys", "Notes"]);
    expect(doc.querySelector("td[style*='text-align:center']")?.textContent).toContain("Mathematician");
    expect(doc.querySelector(".task.done")?.textContent).toContain("typeset the common blocks");
    expect(doc.querySelector(".task.open")?.textContent).toContain("keep the dependency list small");
    expect(doc.querySelector("blockquote")?.textContent).toContain("lightweight markup");
    expect(doc.querySelector(".alert-note .alert-label")?.textContent).toBe("Note");
    expect(doc.querySelector(".alert-warning .alert-label")?.textContent).toBe("Warning");
    const link = doc.querySelector<HTMLAnchorElement>('a[href="https://ratatui.rs"]');
    expect(link?.textContent).toContain("Ratatui");
    expect(link?.textContent).not.toContain("https://ratatui.rs");
    expect(link?.querySelector(".arrow")).not.toBeNull();
    expect(doc.querySelector('a[href="notes.md"]')?.textContent).toContain("field notes");
    expect(doc.querySelector('a[href="#tables"]')).not.toBeNull();
    expect(doc.querySelector("del")?.textContent).toContain("not raw source");
    expect(doc.querySelector("em")?.textContent).toBe("emphasis");
    expect(doc.querySelector("strong")?.textContent).toBe("md");
  });

  it("renders a second document instead of reusing the first", () => {
    const first = renderMarkdown("# One\n\nHello\n");
    const second = renderMarkdown("# Two\n\nThere\n");
    expect(first.html).toContain(">One<");
    expect(second.html).toContain(">Two<");
    expect(second.html).not.toContain(">One<");
  });

  it("does not emit raw html from the document", () => {
    const { html: unsafe } = renderMarkdown("Hello <script>alert(1)</script>\n\n<div>x</div>\n");
    expect(unsafe).not.toContain("<script>");
    expect(unsafe).toContain("&lt;script&gt;");
  });
});
