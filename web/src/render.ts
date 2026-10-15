import hljs from "highlight.js/lib/core";
import bash from "highlight.js/lib/languages/bash";
import c from "highlight.js/lib/languages/c";
import cpp from "highlight.js/lib/languages/cpp";
import css from "highlight.js/lib/languages/css";
import diff from "highlight.js/lib/languages/diff";
import go from "highlight.js/lib/languages/go";
import java from "highlight.js/lib/languages/java";
import javascript from "highlight.js/lib/languages/javascript";
import json from "highlight.js/lib/languages/json";
import markdown from "highlight.js/lib/languages/markdown";
import python from "highlight.js/lib/languages/python";
import ruby from "highlight.js/lib/languages/ruby";
import rust from "highlight.js/lib/languages/rust";
import sql from "highlight.js/lib/languages/sql";
import typescript from "highlight.js/lib/languages/typescript";
import xml from "highlight.js/lib/languages/xml";
import yaml from "highlight.js/lib/languages/yaml";
import { Lexer, type Token, type Tokens } from "marked";

import { uniqueSlug } from "./slug";

hljs.registerLanguage("bash", bash);
hljs.registerLanguage("c", c);
hljs.registerLanguage("cpp", cpp);
hljs.registerLanguage("css", css);
hljs.registerLanguage("diff", diff);
hljs.registerLanguage("go", go);
hljs.registerLanguage("java", java);
hljs.registerLanguage("javascript", javascript);
hljs.registerLanguage("json", json);
hljs.registerLanguage("markdown", markdown);
hljs.registerLanguage("python", python);
hljs.registerLanguage("ruby", ruby);
hljs.registerLanguage("rust", rust);
hljs.registerLanguage("sql", sql);
hljs.registerLanguage("typescript", typescript);
hljs.registerLanguage("xml", xml);
hljs.registerLanguage("yaml", yaml);

function lex(src: string): Token[] {
  // A fresh lexer every time. Reusing one appends to the previous token list.
  return new Lexer({ gfm: true, breaks: false }).lex(src) as Token[];
}

export interface Heading {
  level: number;
  text: string;
  slug: string;
}

export interface RenderedDoc {
  html: string;
  headings: Heading[];
}

const ALERTS: Record<string, string> = {
  note: "Note",
  tip: "Tip",
  important: "Important",
  warning: "Warning",
  caution: "Caution",
};

export function renderMarkdown(src: string): RenderedDoc {
  const headings: Heading[] = [];
  const used = new Map<string, number>();
  const html = renderBlocks(lex(src), headings, used);
  return { html, headings };
}

function renderBlocks(tokens: Token[], headings: Heading[], used: Map<string, number>): string {
  return tokens.map((token) => renderBlock(token, headings, used)).join("");
}

function renderBlock(token: Token, headings: Heading[], used: Map<string, number>): string {
  switch (token.type) {
    case "space":
      return "";
    case "heading": {
      const heading = token as Tokens.Heading;
      const text = inlinePlain(heading.tokens);
      const slug = uniqueSlug(text, used);
      headings.push({ level: heading.depth, text, slug });
      const inner = renderInlines(heading.tokens);
      return `<h${heading.depth} id="${escapeAttr(slug)}">${inner}</h${heading.depth}>`;
    }
    case "paragraph":
      return `<p>${renderInlines((token as Tokens.Paragraph).tokens)}</p>`;
    case "text":
      return `<p>${renderInlines((token as Tokens.Text).tokens ?? [{ type: "text", text: token.text, raw: token.raw } as Token])}</p>`;
    case "code":
      return renderCode(token as Tokens.Code);
    case "blockquote":
      return renderQuote(token as Tokens.Blockquote, headings, used);
    case "list":
      return renderList(token as Tokens.List, headings, used);
    case "table":
      return renderTable(token as Tokens.Table);
    case "hr":
      return `<hr />`;
    case "html":
      return `<pre class="html-raw">${escapeHtml(token.text)}</pre>`;
    default:
      return "";
  }
}

function renderQuote(token: Tokens.Blockquote, headings: Heading[], used: Map<string, number>): string {
  const tokens = token.tokens ?? [];
  const first = tokens[0];
  if (first && first.type === "paragraph") {
    const paragraph = first as Tokens.Paragraph;
    const match = /^\[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\]\s*/i.exec(paragraph.text ?? "");
    if (match) {
      const kind = match[1].toLowerCase();
      const label = ALERTS[kind] ?? kind;
      const inlines = stripAlertPrefix(paragraph.tokens, match[0]);
      const body = `<p>${renderInlines(inlines)}</p>${renderBlocks(tokens.slice(1), headings, used)}`;
      return `<blockquote class="alert alert-${kind}"><p class="alert-label">${label}</p>${body}</blockquote>`;
    }
  }
  return `<blockquote>${renderBlocks(tokens, headings, used)}</blockquote>`;
}

function stripAlertPrefix(tokens: Token[], prefix: string): Token[] {
  let remaining = prefix;
  const next: Token[] = [];
  for (const token of tokens) {
    if (remaining && token.type === "text" && token.text.startsWith(remaining)) {
      const text = token.text.slice(remaining.length);
      remaining = "";
      if (text) {
        const sliced = { ...token, text, raw: text };
        delete (sliced as Tokens.Text).tokens;
        next.push(sliced);
      }
      continue;
    }
    if (remaining && token.type === "text" && remaining.startsWith(token.text)) {
      remaining = remaining.slice(token.text.length);
      continue;
    }
    next.push(token);
  }
  return next;
}

function renderList(token: Tokens.List, headings: Heading[], used: Map<string, number>): string {
  const tasks = token.items.length > 0 && token.items.every((item) => item.task);
  const tag = token.ordered ? "ol" : "ul";
  const start = token.ordered && token.start && token.start !== 1 ? ` start="${token.start}"` : "";
  const klass = tasks ? ` class="tasks"` : "";
  const items = token.items
    .map((item) => {
      const body = renderBlocks(item.tokens, headings, used);
      if (item.task) {
        const state = item.checked ? "done" : "open";
        const mark = item.checked ? "✓" : "☐";
        return `<li class="task ${state}"><span class="box" aria-hidden="true">${mark}</span><div class="task-body">${body}</div></li>`;
      }
      return `<li>${body}</li>`;
    })
    .join("");
  return `<${tag}${klass}${start}>${items}</${tag}>`;
}

function renderTable(token: Tokens.Table): string {
  const head = token.header
    .map((cell) => `<th${alignAttr(cell.align)}>${renderInlines(cell.tokens)}</th>`)
    .join("");
  const rows = token.rows
    .map((row) => {
      const cells = row.map((cell) => `<td${alignAttr(cell.align)}>${renderInlines(cell.tokens)}</td>`).join("");
      return `<tr>${cells}</tr>`;
    })
    .join("");
  return `<div class="table-wrap"><table><thead><tr>${head}</tr></thead><tbody>${rows}</tbody></table></div>`;
}

function alignAttr(align: "center" | "left" | "right" | null): string {
  if (!align || align === "left") return "";
  return ` style="text-align:${align}"`;
}

function renderCode(token: Tokens.Code): string {
  const lang = (token.lang ?? "").trim();
  const label = lang ? `<div class="code-lang">${escapeHtml(lang.split(/\s+/)[0] ?? lang)}</div>` : "";
  const lines = token.text.replace(/\n$/, "").split("\n");
  const language = resolveLang(lang);
  const body = lines
    .map((line, index) => {
      const highlighted = language ? highlightLine(line, language) : escapeHtml(line);
      const shown = highlighted.length > 0 ? highlighted : " ";
      return `<div class="code-line"><span class="gutter">${index + 1}</span><span class="code-text">${shown}</span></div>`;
    })
    .join("");
  return `<div class="code-block">${label}<div class="code-body">${body}</div></div>`;
}

function resolveLang(lang: string): string | null {
  if (!lang) return null;
  const name = lang.split(/\s+/)[0]?.toLowerCase() ?? "";
  if (!name || name === "text" || name === "plain" || name === "txt") return null;
  return hljs.getLanguage(name) ? name : null;
}

function highlightLine(line: string, language: string): string {
  try {
    return hljs.highlight(line, { language, ignoreIllegals: true }).value;
  } catch {
    return escapeHtml(line);
  }
}

function renderInlines(tokens: Token[] | undefined): string {
  if (!tokens) return "";
  return tokens.map(renderInline).join("");
}

function renderInline(token: Token): string {
  switch (token.type) {
    case "escape":
      return escapeHtml(token.text);
    case "html":
      return escapeHtml(token.text);
    case "text": {
      const text = token as Tokens.Text;
      if (text.tokens && text.tokens.length > 0) return renderInlines(text.tokens);
      return escapeHtml(text.text);
    }
    case "strong":
      return `<strong>${renderInlines((token as Tokens.Strong).tokens)}</strong>`;
    case "em":
      return `<em>${renderInlines((token as Tokens.Em).tokens)}</em>`;
    case "del":
      return `<del>${renderInlines((token as Tokens.Del).tokens)}</del>`;
    case "codespan":
      return `<code class="inline">${escapeHtml((token as Tokens.Codespan).text)}</code>`;
    case "br":
      return `<br />`;
    case "link": {
      const link = token as Tokens.Link;
      const href = link.href ?? "";
      const inner = renderInlines(link.tokens);
      return renderAnchor(href, link.title, inner);
    }
    case "image": {
      const image = token as Tokens.Image;
      const href = image.href ?? "";
      if (!safeUrl(href)) return escapeHtml(image.text || href);
      const alt = escapeAttr(image.text || "");
      return `<img src="${escapeAttr(href)}" alt="${alt}" />`;
    }
    default:
      return escapeHtml(tokenText(token));
  }
}

function renderAnchor(href: string, title: string | null | undefined, inner: string): string {
  if (!safeUrl(href) && !href.startsWith("#")) {
    return inner;
  }
  const titleAttr = title ? ` title="${escapeAttr(title)}"` : "";
  const external = /^https?:/i.test(href);
  const mail = /^mailto:/i.test(href);
  const extra = external || mail ? ` target="_blank" rel="noopener noreferrer"` : "";
  const klass = external ? ` class="external"` : "";
  const arrow = external ? `<span class="arrow" aria-hidden="true">↗</span>` : "";
  return `<a href="${escapeAttr(href)}"${klass}${titleAttr}${extra}>${inner}${arrow}</a>`;
}

function safeUrl(href: string): boolean {
  if (href.startsWith("#")) return true;
  if (href.startsWith("/") || href.startsWith("./") || href.startsWith("../")) return true;
  if (/^[a-z0-9.+-]+:/i.test(href)) {
    return /^(https?:|mailto:)/i.test(href);
  }
  return href.length > 0 && !href.startsWith("//");
}

function inlinePlain(tokens: Token[] | undefined): string {
  if (!tokens) return "";
  return tokens.map(plainToken).join("");
}

function plainToken(token: Token): string {
  switch (token.type) {
    case "text":
    case "escape":
    case "codespan":
      return (token as Tokens.Text).tokens?.length
        ? inlinePlain((token as Tokens.Text).tokens)
        : token.text;
    case "strong":
    case "em":
    case "del":
    case "link":
      return inlinePlain((token as Tokens.Strong).tokens);
    case "br":
      return " ";
    case "image":
      return (token as Tokens.Image).text;
    default:
      return tokenText(token);
  }
}

function tokenText(token: Token): string {
  return "text" in token && typeof token.text === "string" ? token.text : "";
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function escapeAttr(value: string): string {
  return escapeHtml(value).replace(/"/g, "&quot;");
}
