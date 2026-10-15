/** A lowercase query ignores ASCII case, matching the terminal reader. */
export function foldsAsciiCase(query: string): boolean {
  return query.length > 0 && !/[A-Z]/.test(query);
}

export function foldAscii(value: string): string {
  return value.replace(/[A-Z]/g, (ch) => ch.toLowerCase());
}

export function clearHits(root: ParentNode): void {
  const marks = [...root.querySelectorAll("mark.hit")];
  for (const mark of marks) {
    const parent = mark.parentNode;
    if (!parent) continue;
    const text = document.createTextNode(mark.textContent ?? "");
    parent.replaceChild(text, mark);
    parent.normalize();
  }
}

/** Wrap query matches in `<mark class="hit">` and return them in document order. */
export function collectHits(root: HTMLElement, query: string): HTMLElement[] {
  clearHits(root);
  if (!query) return [];
  const fold = foldsAsciiCase(query);
  const needle = fold ? foldAscii(query) : query;
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      const parent = node.parentElement;
      if (!parent) return NodeFilter.FILTER_REJECT;
      if (parent.closest(".gutter, script, style")) return NodeFilter.FILTER_REJECT;
      return NodeFilter.FILTER_ACCEPT;
    },
  });
  const nodes: Text[] = [];
  let current = walker.nextNode();
  while (current) {
    nodes.push(current as Text);
    current = walker.nextNode();
  }

  const hits: HTMLElement[] = [];
  for (const textNode of nodes) {
    const value = textNode.nodeValue ?? "";
    if (!value) continue;
    const hay = fold ? foldAscii(value) : value;
    const ranges: Array<[number, number]> = [];
    let from = 0;
    while (from < hay.length) {
      const index = hay.indexOf(needle, from);
      if (index < 0) break;
      ranges.push([index, index + needle.length]);
      from = index + Math.max(needle.length, 1);
    }
    if (ranges.length === 0) continue;
    let node = textNode;
    const created: HTMLElement[] = [];
    for (let i = ranges.length - 1; i >= 0; i -= 1) {
      const [start, end] = ranges[i];
      node.splitText(end);
      const mid = node.splitText(start);
      const mark = document.createElement("mark");
      mark.className = "hit";
      mid.parentNode?.replaceChild(mark, mid);
      mark.appendChild(mid);
      created.push(mark);
    }
    hits.push(...created.reverse());
  }
  hits.forEach((el, index) => {
    el.dataset.hit = String(index);
  });
  return hits;
}

export function setCurrentHit(hits: HTMLElement[], index: number): number {
  if (hits.length === 0) return -1;
  const next = ((index % hits.length) + hits.length) % hits.length;
  hits.forEach((el, i) => el.classList.toggle("current", i === next));
  hits[next]?.scrollIntoView({ block: "center" });
  return next;
}
