/** Same rules as `slugify` in the Rust reader (`crates/md/src/open.rs`). */
export function slugify(text: string): string {
  let out = "";
  let dash = false;
  for (const c of text) {
    if (c === "'" || c === "\u2019") continue;
    if (/^[0-9A-Za-z]$/.test(c)) {
      out += c.toLowerCase();
      dash = false;
    } else if (/^\p{L}$|^\p{N}$/u.test(c)) {
      out += c.toLowerCase();
      dash = false;
    } else if (!dash && out.length > 0) {
      out += "-";
      dash = true;
    }
  }
  while (out.endsWith("-")) out = out.slice(0, -1);
  return out;
}

export function uniqueSlug(text: string, used: Map<string, number>): string {
  const base = slugify(text) || "section";
  const seen = used.get(base) ?? 0;
  used.set(base, seen + 1);
  return seen === 0 ? base : `${base}-${seen}`;
}
