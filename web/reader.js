// Continuous reading: a contents page, then chapter after chapter as you scroll.

const AHEAD = 2000; // how far past the bottom of the screen chapters are loaded, in pixels

export class Reader {
  // version() is "kjv", "both", or "esv"; esv(c) resolves to a chapter's ESV verses by number.
  constructor(box, scroller, { ask, meta, verseHtml, onVerse, version, versionLabel, esv, esc }) {
    this.box = box;
    this.scroller = scroller;
    this.ask = ask;
    this.meta = meta;
    this.verseHtml = verseHtml;
    this.onVerse = onVerse;
    this.version = version;
    this.versionLabel = versionLabel;
    this.esv = esv;
    this.esc = esc;
    this.first = -1; // first and last chapter currently rendered
    this.last = -1;
    this.loading = false;
    this.token = 0;
    this.sentinel = document.createElement("div");
    this.sentinel.className = "reader-end";
    new IntersectionObserver((e) => e[0].isIntersecting && this.appendNext(), {
      root: scroller,
      rootMargin: `${AHEAD}px`,
    }).observe(this.sentinel);
    let pending = false;
    scroller.addEventListener("scroll", () => {
      if (pending || this.first < 0) return;
      pending = true;
      setTimeout(() => {
        pending = false;
        this.track();
      }, 120);
    }, { passive: true });
  }

  chapterCount() {
    return this.meta.chapterStart.length;
  }

  chapterOf(v) {
    const s = this.meta.chapterStart;
    let lo = 0, hi = s.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (s[mid] <= v) lo = mid; else hi = mid - 1;
    }
    return lo;
  }

  async chapterHtml(c) {
    const m = this.meta;
    const ch = await this.ask("chapter", { verse: m.chapterStart[c] });
    const book = m.books[m.chapterBook[c]][1];
    const single = !m.chapterBook.some((b, i) => b === m.chapterBook[c] && i !== c);
    let version = this.version();
    let esv = null;
    let note = "";
    if (version !== "kjv") {
      try {
        esv = await this.esv(c);
        note = `ESV · <a href="https://www.esv.org" target="_blank" rel="noopener">esv.org</a>`;
      } catch (e) {
        version = "kjv";
        note = `The ESV didn't load (${this.esc(e.message)}). This is the King James.`;
      }
    }
    const last = ch.verses.length;
    const esvText = (n) => {
      // Where the ESV numbers a verse the King James doesn't have, it joins the last one.
      const parts = [esv.get(n) || ""];
      if (n === last) for (const [k, text] of esv) if (k > last) parts.push(text);
      return `<span class="esv">${this.esc(parts.join(" "))}</span>`;
    };
    const verses = ch.verses.map((x) => {
      const para = x.seg[0]?.[0]?.startsWith("¶") ? " para" : "";
      const kjv = this.verseHtml(x.seg);
      const text = version === "kjv" ? kjv : version === "esv" ? esvText(x.v) : `<span class="kjv">${kjv}</span>${esvText(x.v)}`;
      return `<p class="vl${para}" data-v="${x.id}"><sup>${x.v}</sup>${text}</p>`;
    }).join("");
    const heads = version === "both" ? `<p class="versions"><span>King James</span><span>ESV</span></p>` : "";
    return `<section class="chapter" data-c="${c}" data-version="${version}">
      <header><span class="book">${book}</span>${single ? "" : `<span class="num">${m.chapterNum[c]}</span>`}</header>
      ${ch.title && version !== "esv" ? `<p class="title">${ch.title}</p>` : ""}
      <div class="verses">${heads}${verses}</div>${note ? `<p class="esv-note">${note}</p>` : ""}</section>`;
  }

  // Open the reader at verse v: its chapter, scrolled so v sits near the top.
  async open(v) {
    const token = ++this.token;
    const c = this.chapterOf(v);
    const html = await this.chapterHtml(c);
    if (token !== this.token) return;
    this.first = this.last = c;
    const prev = c > 0
      ? `<button type="button" class="prev-chapter" data-go="${this.meta.chapterStart[c - 1]}">${this.label(c - 1)}</button>`
      : "";
    const label = this.versionLabel();
    const version = label ? `<button type="button" data-act="version" title="Change translation">${label}</button>` : "";
    // The ESV's terms keep it to one chapter on a page, so it gets a link onward instead of a longer scroll.
    const next = this.version() !== "kjv" && c + 1 < this.chapterCount()
      ? `<nav class="reader-nav"><span></span>${this.chapterLink(c + 1, this.label(c + 1))}</nav>` : "";
    this.box.innerHTML = `<nav class="reader-nav"><button type="button" data-act="contents">contents</button>${version}${prev}</nav>${html}${next}`;
    this.box.append(this.sentinel);
    this.mark(v);
    const el = this.box.querySelector(`[data-v="${v}"]`);
    if (el && v !== this.meta.chapterStart[c]) el.scrollIntoView({ block: "start" });
    else this.scroller.scrollTop = 0;
    this.appendNext();
  }

  label(c) {
    const m = this.meta;
    return `${m.books[m.chapterBook[c]][1]} ${m.chapterNum[c]}`;
  }

  async appendNext() {
    if (this.version() !== "kjv") return;
    if (this.loading || this.last < 0 || this.last + 1 >= this.chapterCount()) return;
    this.loading = true;
    const token = this.token;
    const html = await this.chapterHtml(this.last + 1);
    this.loading = false;
    if (token !== this.token) return;
    this.last++;
    this.sentinel.insertAdjacentHTML("beforebegin", html);
    // Short chapters can leave the end still close; keep going so a fast scroll never runs out of page.
    const shown = this.sentinel.offsetParent !== null;
    if (shown && this.sentinel.getBoundingClientRect().top < this.scroller.getBoundingClientRect().bottom + AHEAD) this.appendNext();
  }

  mark(v) {
    this.box.querySelector(".vl.here")?.classList.remove("here");
    this.box.querySelector(`[data-v="${v}"]`)?.classList.add("here");
  }

  // The verse at the top of the screen is where you are.
  track() {
    const r = this.scroller.getBoundingClientRect();
    const el = document.elementFromPoint(r.left + r.width / 2, r.top + 24)?.closest(".vl");
    if (el) this.onVerse(Number(el.dataset.v), false);
  }

  step(dir) {
    const all = [...this.box.querySelectorAll(".vl")];
    const i = all.findIndex((el) => el.classList.contains("here"));
    const next = all[Math.max(0, Math.min(all.length - 1, (i < 0 ? 0 : i + dir)))];
    if (!next) return;
    this.mark(Number(next.dataset.v));
    next.scrollIntoView({ block: "nearest" });
    this.onVerse(Number(next.dataset.v), true);
  }

  chapterStep(dir, from) {
    const c = Math.max(0, Math.min(this.chapterCount() - 1, this.chapterOf(from) + dir));
    return this.meta.chapterStart[c];
  }

  // A real link to chapter c, so a new tab works; data-go lets a plain click stay in the app.
  chapterLink(c, label) {
    const m = this.meta;
    return `<a href="?read=${m.books[m.chapterBook[c]][0]}.${m.chapterNum[c]}" data-go="${m.chapterStart[c]}">${label}</a>`;
  }

  // Every book, each a link to its first chapter.
  booksHtml() {
    const m = this.meta;
    const first = [];
    m.chapterBook.forEach((b, c) => (first[b] ??= c));
    const group = (title, from, to) =>
      `<h2>${title}</h2><ol class="books">${first.slice(from, to).map((c, i) => `<li>${this.chapterLink(c, m.books[from + i][1])}</li>`).join("")}</ol>`;
    return group("The Old Testament", 0, 39) + group("The New Testament", 39, 66);
  }

  contents() {
    ++this.token;
    this.first = this.last = -1;
    this.box.innerHTML = `<div class="contents">${this.booksHtml()}</div>`;
    this.scroller.scrollTop = 0;
  }
}
