import "@fontsource/ibm-plex-sans/400.css";
import "@fontsource/ibm-plex-sans/600.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/500.css";

import { initialState, reduce, type KeyState, type ThemeName } from "./keys";
import { renderMarkdown, type Heading } from "./render";
import { collectHits, setCurrentHit } from "./search";
import "./style.css";

interface Source {
  title: string;
  markdown: string;
  /** Base URL for relative links, when the document was fetched. */
  base: string | null;
}

interface HistoryEntry {
  source: Source;
  scroll: number;
}

const appEl = document.querySelector<HTMLElement>(".app")!;
const article = document.querySelector<HTMLElement>("#doc")!;
const outline = document.querySelector<HTMLElement>("#outline")!;
const outlineList = document.querySelector<HTMLElement>("#outline-list")!;
const statusText = document.querySelector<HTMLElement>("#status-text")!;
const fileLabel = document.querySelector<HTMLElement>("#file-label")!;
const searchBar = document.querySelector<HTMLElement>("#search")!;
const searchInput = document.querySelector<HTMLInputElement>("#search-input")!;
const help = document.querySelector<HTMLElement>("#help")!;
const drop = document.querySelector<HTMLElement>("#drop")!;
const fileInput = document.querySelector<HTMLInputElement>("#file-input")!;
const themeButton = document.querySelector<HTMLButtonElement>("#theme-button")!;

const state: KeyState = initialState();
let source: Source = { title: "showcase.md", markdown: "", base: new URL("/showcase.md", location.origin).href };
let headings: Heading[] = [];
let hits: HTMLElement[] = [];
let hitIndex = -1;
let committedQuery = "";
const history: HistoryEntry[] = [];
let statusDetail = "";

function lineHeight(): number {
  return 32;
}

function applyTheme(theme: ThemeName): void {
  document.documentElement.dataset.theme = theme;
  themeButton.textContent = theme;
}

function setOutline(open: boolean): void {
  appEl.classList.toggle("outline-closed", !open);
  outline.hidden = !open && window.matchMedia("(max-width: 800px)").matches ? true : !open;
  if (window.matchMedia("(max-width: 800px)").matches) {
    outline.hidden = !open;
    appEl.classList.toggle("outline-open", open);
  }
}

function showDocument(next: Source, pushHistory: boolean): void {
  if (pushHistory && source.markdown) {
    history.push({ source: { ...source }, scroll: window.scrollY });
  }
  source = next;
  const rendered = renderMarkdown(next.markdown);
  headings = rendered.headings;
  article.innerHTML = rendered.html;
  fileLabel.textContent = next.title;
  document.title = `${next.title} — md`;
  renderOutline();
  hits = [];
  hitIndex = -1;
  if (committedQuery) hits = collectHits(article, committedQuery);
  statusDetail = "";
  updateStatus();
  window.scrollTo(0, 0);
}

function renderOutline(): void {
  if (headings.length === 0) {
    outlineList.innerHTML = `<p class="outline-empty">No headings</p>`;
    return;
  }
  outlineList.innerHTML = headings
    .map(
      (heading) =>
        `<a href="#${heading.slug}" data-level="${heading.level}" data-slug="${heading.slug}">${escapeText(heading.text)}</a>`,
    )
    .join("");
}

function updateStatus(): void {
  const max = Math.max(1, document.documentElement.scrollHeight - window.innerHeight);
  const pct = Math.round((window.scrollY / max) * 100);
  const current = currentHeading();
  const match =
    committedQuery && hits.length > 0
      ? `${hitIndex + 1}/${hits.length}`
      : committedQuery
        ? "no matches"
        : "";
  const parts = [source.title, `${pct}%`, current, match, statusDetail].filter(Boolean);
  statusText.textContent = parts.join("   ");
  const links = [...outlineList.querySelectorAll<HTMLAnchorElement>("a")];
  for (const link of links) {
    link.classList.toggle("active", link.dataset.slug === slugOf(current));
  }
}

function currentHeading(): string {
  let name = headings[0]?.text ?? "";
  const y = window.scrollY + 72;
  for (const heading of headings) {
    const el = document.getElementById(heading.slug);
    if (!el) continue;
    if (el.offsetTop <= y) name = heading.text;
  }
  return name;
}

function slugOf(text: string): string {
  return headings.find((heading) => heading.text === text)?.slug ?? "";
}

function scrollByLines(dir: 1 | -1, lines: number): void {
  window.scrollBy(0, dir * lineHeight() * lines);
}

function scrollPage(dir: 1 | -1, fraction: number): void {
  const amount = Math.max(lineHeight() * 4, (window.innerHeight - 80) * fraction);
  window.scrollBy(0, dir * amount);
}

function jumpHeading(dir: 1 | -1): void {
  const y = window.scrollY + 8;
  const nodes = headings
    .map((heading) => ({ heading, el: document.getElementById(heading.slug) }))
    .filter((item): item is { heading: Heading; el: HTMLElement } => item.el !== null);
  if (dir > 0) {
    const next = nodes.find((item) => item.el.offsetTop > y + 4);
    next?.el.scrollIntoView();
  } else {
    const prev = [...nodes].reverse().find((item) => item.el.offsetTop < y - 4);
    prev?.el.scrollIntoView();
  }
}

searchBar.addEventListener("submit", (event) => {
  event.preventDefault();
});

function openSearch(): void {
  searchBar.hidden = false;
  searchInput.value = committedQuery;
  searchInput.focus();
  searchInput.select();
}

function closeSearch(commit: boolean): void {
  searchBar.hidden = true;
  searchInput.blur();
  if (commit) {
    committedQuery = searchInput.value;
    hits = collectHits(article, committedQuery);
    hitIndex = hits.length ? setCurrentHit(hits, 0) : -1;
    statusDetail = committedQuery && hits.length === 0 ? "no matches" : "";
  } else {
    hits = collectHits(article, committedQuery);
    if (hitIndex >= 0 && hits.length) hitIndex = setCurrentHit(hits, hitIndex);
  }
  updateStatus();
}

function moveHit(dir: 1 | -1): void {
  if (!committedQuery) {
    statusDetail = "no search";
    updateStatus();
    return;
  }
  if (hits.length === 0) {
    statusDetail = "no matches";
    updateStatus();
    return;
  }
  hitIndex = setCurrentHit(hits, hitIndex + dir);
  statusDetail = "";
  updateStatus();
}

async function loadUrl(url: string, title: string, pushHistory: boolean): Promise<void> {
  statusDetail = "loading…";
  updateStatus();
  try {
    const response = await fetch(url);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const markdown = await response.text();
    showDocument({ title, markdown, base: new URL(url, location.href).href }, pushHistory);
  } catch (error) {
    statusDetail = `could not load ${title}: ${error instanceof Error ? error.message : "failed"} (CORS?)`;
    updateStatus();
  }
}

function loadText(title: string, markdown: string): void {
  showDocument({ title, markdown, base: null }, true);
}

async function followHref(href: string): Promise<void> {
  const hash = href.indexOf("#");
  const path = hash >= 0 ? href.slice(0, hash) : href;
  const anchor = hash >= 0 ? href.slice(hash + 1) : "";
  if (!path) {
    const target = document.getElementById(anchor);
    target?.scrollIntoView();
    return;
  }
  if (!source.base) {
    statusDetail = "relative links need a fetched file, not a pasted buffer";
    updateStatus();
    return;
  }
  const next = new URL(path, source.base);
  await loadUrl(next.href, next.pathname.split("/").pop() || next.href, true);
  if (anchor) {
    requestAnimationFrame(() => document.getElementById(anchor)?.scrollIntoView());
  }
}

function goBack(): void {
  const previous = history.pop();
  if (!previous) {
    statusDetail = "nothing to go back to";
    updateStatus();
    return;
  }
  const scroll = previous.scroll;
  showDocument(previous.source, false);
  window.scrollTo(0, scroll);
}

function escapeText(value: string): string {
  return value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function readFile(file: File): void {
  const reader = new FileReader();
  reader.onload = () => {
    loadText(file.name, String(reader.result ?? ""));
  };
  reader.onerror = () => {
    statusDetail = `could not read ${file.name}`;
    updateStatus();
  };
  reader.readAsText(file);
}

document.querySelector("#open-button")?.addEventListener("click", () => fileInput.click());
document.querySelector("#showcase-button")?.addEventListener("click", () => {
  void loadUrl("/showcase.md", "showcase.md", true);
});
themeButton.addEventListener("click", () => {
  state.theme = state.theme === "ink" ? "paper" : "ink";
  applyTheme(state.theme);
});
document.querySelector("#outline-button")?.addEventListener("click", () => {
  state.outline = !state.outline;
  setOutline(state.outline);
});
fileInput.addEventListener("change", () => {
  const file = fileInput.files?.[0];
  if (file) readFile(file);
  fileInput.value = "";
});

article.addEventListener("click", (event) => {
  const anchor = (event.target as HTMLElement | null)?.closest("a");
  if (!anchor) return;
  const href = anchor.getAttribute("href") ?? "";
  if (/^(https?:|mailto:)/i.test(href)) return;
  if (href.startsWith("#")) return;
  const path = href.split("#")[0] ?? "";
  if (/\.(md|markdown)$/i.test(path)) {
    event.preventDefault();
    void followHref(href);
  }
});

outlineList.addEventListener("click", (event) => {
  const anchor = (event.target as HTMLElement | null)?.closest("a");
  if (!anchor) return;
  if (window.matchMedia("(max-width: 800px)").matches) {
    state.outline = false;
    setOutline(false);
  }
});

window.addEventListener("scroll", () => updateStatus(), { passive: true });

window.addEventListener("keydown", (event) => {
  const target = event.target as HTMLElement | null;
  const typing =
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target?.isContentEditable;
  if (typing && target !== searchInput) return;
  if (event.metaKey || event.altKey) return;

  if (state.mode === "search") {
    if (event.key === "Enter") {
      event.preventDefault();
      const result = reduce(state, { key: "Enter" });
      Object.assign(state, result.state);
      closeSearch(true);
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      const result = reduce(state, { key: "Escape" });
      Object.assign(state, result.state);
      closeSearch(false);
      return;
    }
    return;
  }

  if (event.key === "Backspace" && state.mode === "read") {
    event.preventDefault();
    goBack();
    return;
  }

  const result = reduce(state, { key: event.key, ctrl: event.ctrlKey });
  if (result.command.type === "none" && !result.state.pendingG && event.key.length === 1 && !event.ctrlKey) {
    return;
  }
  if (["j", "k", "g", "G", "/", " ", "d", "u", "b", "o", "t", "n", "N", "?", "[", "]"].includes(event.key) || event.ctrlKey) {
    event.preventDefault();
  }
  Object.assign(state, result.state);
  switch (result.command.type) {
    case "line":
      scrollByLines(result.command.dir, 1);
      break;
    case "half":
      scrollPage(result.command.dir, 0.5);
      break;
    case "page":
      scrollPage(result.command.dir, 1);
      break;
    case "top":
      window.scrollTo(0, 0);
      break;
    case "bottom":
      window.scrollTo(0, document.documentElement.scrollHeight);
      break;
    case "searchStart":
      openSearch();
      break;
    case "find":
      moveHit(result.command.dir);
      break;
    case "heading":
      jumpHeading(result.command.dir);
      break;
    case "toggleOutline":
      setOutline(state.outline);
      break;
    case "cycleTheme":
      applyTheme(state.theme);
      break;
    case "openHelp":
      help.hidden = false;
      break;
    case "closeHelp":
      help.hidden = true;
      break;
    default:
      break;
  }
});

window.addEventListener("paste", (event) => {
  if (state.mode === "search") return;
  const target = event.target as HTMLElement | null;
  if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) return;
  const file = [...(event.clipboardData?.files ?? [])].find((item) =>
    /\.(md|markdown|txt)$/i.test(item.name) || item.type.startsWith("text/"),
  );
  if (file) {
    event.preventDefault();
    readFile(file);
    return;
  }
  const text = event.clipboardData?.getData("text/plain") ?? "";
  if (text.trim().length > 0) {
    event.preventDefault();
    loadText("paste", text);
  }
});

let dragDepth = 0;
window.addEventListener("dragenter", (event) => {
  event.preventDefault();
  dragDepth += 1;
  drop.hidden = false;
});
window.addEventListener("dragover", (event) => {
  event.preventDefault();
});
window.addEventListener("dragleave", () => {
  dragDepth = Math.max(0, dragDepth - 1);
  if (dragDepth === 0) drop.hidden = true;
});
window.addEventListener("drop", (event) => {
  event.preventDefault();
  dragDepth = 0;
  drop.hidden = true;
  const file = event.dataTransfer?.files?.[0];
  if (file) readFile(file);
});

help.addEventListener("click", (event) => {
  if (event.target === help) {
    help.hidden = true;
    state.mode = "read";
  }
});

function boot(): void {
  const stored = localStorage.getItem("md-theme");
  if (stored === "paper" || stored === "ink") state.theme = stored;
  applyTheme(state.theme);
  const narrow = window.matchMedia("(max-width: 800px)").matches;
  if (narrow) {
    state.outline = false;
    setOutline(false);
  } else {
    setOutline(true);
  }
  const params = new URLSearchParams(location.search);
  const remote = params.get("url");
  if (remote) {
    const title = remote.split("/").pop() || remote;
    void loadUrl(remote, title, false);
    return;
  }
  void loadUrl("/showcase.md", "showcase.md", false);
}

themeButton.addEventListener("click", () => {
  localStorage.setItem("md-theme", state.theme);
});

window.addEventListener("beforeunload", () => {
  localStorage.setItem("md-theme", state.theme);
});

boot();
