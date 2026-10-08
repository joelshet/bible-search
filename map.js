// The whole Bible as one zoomable map. Every verse is a rectangle sized by its
// length, so at full zoom each one holds its own text at one shared size.

const LINE = 1.28;         // line height, in font sizes
const MIN_TEXT_PX = 5.5;   // below this, verses are colored blocks
const MAX_TEXT_PX = 26;    // zoom stops when verse text reaches this size

export class BibleMap {
  constructor(canvas, { layout, aspect, meta, onPick, onHover }) {
    this.canvas = canvas;
    this.ctx = canvas.getContext("2d");
    this.meta = meta;
    this.text = meta.text;
    this.onPick = onPick;
    this.onHover = onHover;
    this.books = meta.books.length;
    this.chapters = meta.chapterStart.length;
    this.rects = layout; // [books][chapters][verses] × (x, y, w, h), in an aspect × 1 box
    this.aspect = aspect;
    this.verses = (layout.length / 4) - this.books - this.chapters;
    this.hits = null;
    this.selected = -1;
    this.wrapCache = new Map();
    this.cam = { x: aspect / 2, y: 0.5, k: 1 };
    this.family = "Georgia, serif";
    this.labelFamily = '"EB Garamond", Georgia, serif';
    this.chapterBooks = Array.from({ length: this.books }, () => []);
    meta.chapterStart.forEach((_, c) => this.chapterBooks[meta.chapterBook[c]].push(c));
    this.verseChars = new Float32Array(this.verses);
    for (let v = 0; v < this.verses; v++) this.verseChars[v] = this.text[v].length;
    this.bindInput();
    this.resize();
  }

  rect(i) {
    const r = this.rects;
    return [r[i * 4], r[i * 4 + 1], r[i * 4 + 2], r[i * 4 + 3]];
  }
  bookRect(b) { return this.rect(b); }
  chapterRect(c) { return this.rect(this.books + c); }
  verseRect(v) { return this.rect(this.books + this.chapters + v); }

  chapterRange(c) {
    const s = this.meta.chapterStart;
    return [s[c], c + 1 < s.length ? s[c + 1] : this.verses];
  }

  setColors(colors) {
    if (JSON.stringify(colors) === JSON.stringify(this.colors)) return;
    this.colors = colors;
    this.glowStale = true;
    this.draw();
  }

  setFont(family) {
    if (family === this.family) return;
    this.family = family;
    this.remeasure();
  }

  // Verses wrapped before a web font finished loading were measured in its fallback.
  remeasure() {
    this.wrapCache.clear();
    this.draw();
  }

  setHits(hits) {
    this.hits = hits;
    this.glowStale = true;
    this.draw();
  }

  setSelected(v) {
    this.selected = v;
    this.draw();
  }

  // Swap in a layout drawn for a different shape of space.
  setLayout(layout, aspect) {
    this.rects = layout;
    this.aspect = aspect;
    this.typicalF = 0;
    this.wrapCache.clear();
    this.cam = { x: aspect / 2, y: 0.5, k: this.fitK() };
    this.glowStale = true;
    this.draw();
  }

  fitK() {
    return Math.min(this.w / this.aspect, this.h) * 0.96;
  }

  resize() {
    const dpr = window.devicePixelRatio || 1;
    const { width, height } = this.canvas.getBoundingClientRect();
    if (!width || !height) return;
    const firstFit = !this.w;
    const oldFit = this.w ? this.fitK() : 0;
    this.w = width;
    this.h = height;
    this.dpr = dpr;
    this.canvas.width = Math.round(width * dpr);
    this.canvas.height = Math.round(height * dpr);
    if (firstFit) this.cam.k = this.fitK();
    else this.cam.k *= this.fitK() / oldFit;
    this.clamp();
    this.draw();
  }

  // Font size (in map units) that fills verse v's rectangle, and its wrapped lines.
  wrap(v) {
    let entry = this.wrapCache.get(v);
    if (entry) return entry;
    const [, , w, h] = this.verseRect(v);
    const ctx = this.ctx;
    const ref = 100;
    ctx.font = `${ref}px ${this.family}`;
    const words = this.text[v].split(" ");
    let f = Math.sqrt((w * h) / (Math.max(this.verseChars[v], 8) * 0.62 * LINE));
    let lines = [];
    for (let tries = 0; tries < 14; tries++) {
      const maxW = (w * 0.92 / f) * ref;
      lines = [];
      let line = "";
      for (const word of words) {
        const next = line ? line + " " + word : word;
        if (line && ctx.measureText(next).width > maxW) {
          lines.push(line);
          line = word;
        } else line = next;
      }
      if (line) lines.push(line);
      const widest = Math.max(...lines.map((l) => ctx.measureText(l).width));
      if ((lines.length * LINE + 0.4) * f <= h * 0.94 && widest <= maxW * 1.02) break;
      f *= 0.92;
    }
    entry = { f, lines };
    this.wrapCache.set(v, entry);
    return entry;
  }

  toScreen(x, y) {
    const { cam } = this;
    return [(x - cam.x) * cam.k + this.w / 2, (y - cam.y) * cam.k + this.h / 2];
  }
  toMap(sx, sy) {
    const { cam } = this;
    return [(sx - this.w / 2) / cam.k + cam.x, (sy - this.h / 2) / cam.k + cam.y];
  }

  maxK() {
    // Zoom stops once a typical verse's text is comfortably large.
    if (!this.typicalF) {
      const fs = [];
      for (let v = 0; v < this.verses; v += 97) {
        const [, , w, h] = this.verseRect(v);
        fs.push(Math.sqrt((w * h) / (Math.max(this.verseChars[v], 8) * 0.62 * LINE)));
      }
      fs.sort((a, b) => a - b);
      this.typicalF = fs[fs.length >> 1] * 0.85;
    }
    return MAX_TEXT_PX / this.typicalF;
  }

  clamp() {
    const { cam } = this;
    cam.k = Math.min(Math.max(cam.k, this.fitK() * 0.9), this.maxK());
    const hw = this.w / 2 / cam.k, hh = this.h / 2 / cam.k;
    const a = this.aspect;
    cam.x = hw >= a / 2 ? a / 2 : Math.min(Math.max(cam.x, hw), a - hw);
    cam.y = hh >= 0.5 ? 0.5 : Math.min(Math.max(cam.y, hh), 1 - hh);
  }

  zoomAt(sx, sy, factor) {
    const [mx, my] = this.toMap(sx, sy);
    this.cam.k *= factor;
    this.clamp();
    const [nx, ny] = this.toMap(sx, sy);
    this.cam.x += mx - nx;
    this.cam.y += my - ny;
    this.clamp();
    this.draw();
  }

  zoomBy(factor) {
    this.zoomAt(this.w / 2, this.h / 2, factor);
  }

  reset() {
    this.cam = { x: this.aspect / 2, y: 0.5, k: this.fitK() };
    this.draw();
  }

  // Center a verse at a zoom where its text is readable.
  locate(v) {
    if (v < 0) return;
    const [x, y, w, h] = this.verseRect(v);
    const { f } = this.wrap(v);
    this.cam.x = x + w / 2;
    this.cam.y = y + h / 2;
    this.cam.k = Math.min(16 / f, Math.min(this.w, this.h) * 0.7 / Math.max(w, h));
    this.clamp();
    this.draw();
  }

  pick(sx, sy) {
    const [mx, my] = this.toMap(sx, sy);
    const inside = ([x, y, w, h]) => mx >= x && mx < x + w && my >= y && my < y + h;
    for (let b = 0; b < this.books; b++) {
      if (!inside(this.bookRect(b))) continue;
      for (const c of this.chapterBooks[b]) {
        if (!inside(this.chapterRect(c))) continue;
        const [s, e] = this.chapterRange(c);
        for (let v = s; v < e; v++) if (inside(this.verseRect(v))) return v;
      }
    }
    return -1;
  }

  draw() {
    if (this.frame) return;
    this.frame = requestAnimationFrame(() => {
      this.frame = 0;
      this.paint();
    });
  }

  // A soft halo under the matches, drawn once per search at overview scale and blurred.
  // Zoomed out, the Bible reads like a night photo where the matching verses are lit.
  // The tiles are drawn sharp, then blurred in one pass: a filter set while drawing them
  // would blur every tile separately, which takes seconds on a broad search.
  renderGlow() {
    this.glowStale = false;
    this.glow = null;
    if (!this.hits) return;
    const size = Math.round(900 / Math.max(1, this.aspect));
    const sharp = (this.glowSharp ||= document.createElement("canvas"));
    const soft = (this.glowSoft ||= document.createElement("canvas"));
    sharp.width = soft.width = Math.round(size * this.aspect);
    sharp.height = soft.height = size;
    let ctx = sharp.getContext("2d");
    ctx.fillStyle = this.colors.glow;
    for (let v = 0; v < this.verses; v++) {
      const hit = this.hits[v];
      if (!hit) continue;
      const [x, y, w, h] = this.verseRect(v);
      ctx.globalAlpha = 0.25 + 0.75 * (hit / 255);
      ctx.fillRect(x * size - 1, y * size - 1, Math.max(w * size, 1) + 2, Math.max(h * size, 1) + 2);
    }
    ctx = soft.getContext("2d");
    ctx.filter = "blur(5px)";
    ctx.drawImage(sharp, 0, 0);
    this.glow = soft;
  }

  paint() {
    const { ctx, cam, colors } = this;
    if (!colors || !this.w) return;
    this.maxK();
    const dpr = this.dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.globalAlpha = 1;
    ctx.globalCompositeOperation = "source-over";
    ctx.fillStyle = colors.paper;
    ctx.fillRect(0, 0, this.w, this.h);
    const [vx0, vy0] = this.toMap(0, 0);
    const [vx1, vy1] = this.toMap(this.w, this.h);
    const visible = ([x, y, w, h]) => x < vx1 && x + w > vx0 && y < vy1 && y + h > vy0;
    const k = cam.k;
    const sx = (x) => (x - cam.x) * k + this.w / 2;
    const sy = (y) => (y - cam.y) * k + this.h / 2;
    const reading = this.typicalF * k >= MIN_TEXT_PX;

    const visibleBooks = [];
    for (let b = 0; b < this.books; b++) if (visible(this.bookRect(b))) visibleBooks.push(b);
    const visibleChapters = [];
    for (const b of visibleBooks) {
      for (const c of this.chapterBooks[b]) if (visible(this.chapterRect(c))) visibleChapters.push(c);
    }

    // The mosaic: one tile per verse, with a hairline of paper between tiles.
    const tiles = [new Path2D(), new Path2D()];
    const bands = 6;
    const lit = Array.from({ length: bands }, () => new Path2D());
    const usedBand = new Array(bands).fill(false);
    for (const c of visibleChapters) {
      const tone = this.meta.chapterBook[c] % 2;
      const [s, e] = this.chapterRange(c);
      for (let v = s; v < e; v++) {
        const r = this.verseRect(v);
        if (!visible(r)) continue;
        const w = r[2] * k, h = r[3] * k;
        const gap = Math.min(0.5, w * 0.18, h * 0.18) + (reading ? 1 : 0);
        const x = sx(r[0]) + gap, y = sy(r[1]) + gap;
        const tw = Math.max(w - gap * 2, 0.6), th = Math.max(h - gap * 2, 0.6);
        const hit = this.hits ? this.hits[v] : 0;
        if (hit && !reading) {
          const band = Math.min(bands - 1, Math.floor((hit / 256) * bands));
          lit[band].rect(x, y, tw, th);
          usedBand[band] = true;
        } else tiles[tone].rect(x, y, tw, th);
        if (hit && reading) {
          lit[bands - 1].rect(x, y, tw, th);
          usedBand[bands - 1] = true;
        }
      }
    }
    ctx.fillStyle = colors.tile;
    ctx.fill(tiles[0]);
    ctx.fillStyle = colors.tileAlt;
    ctx.fill(tiles[1]);
    ctx.fillStyle = colors.glow;
    for (let i = 0; i < bands; i++) {
      if (!usedBand[i]) continue;
      ctx.globalAlpha = reading ? 0.13 : 0.35 + 0.65 * ((i + 1) / bands);
      ctx.fill(lit[i]);
    }
    ctx.globalAlpha = 1;

    // The halo fades out as you zoom in, where it would only blur the tiles.
    const fade = 1 - Math.min(1, Math.max(0, (k / this.fitK() - 1.5) / 4));
    if (fade > 0 && this.glowStale) this.renderGlow();
    if (this.glow && fade > 0) {
      ctx.globalCompositeOperation = colors.dark ? "lighter" : "multiply";
      ctx.globalAlpha = (colors.dark ? 0.55 : 0.35) * fade;
      ctx.drawImage(this.glow, sx(0), sy(0), k * this.aspect, k);
      ctx.globalCompositeOperation = "source-over";
      ctx.globalAlpha = 1;
    }

    // Wider paper gaps between chapters and books show structure without drawing lines.
    ctx.strokeStyle = colors.paper;
    ctx.lineWidth = reading ? 3 : 1.2;
    ctx.beginPath();
    for (const c of visibleChapters) {
      const r = this.chapterRect(c);
      if (Math.min(r[2], r[3]) * k < 10) continue;
      ctx.rect(sx(r[0]), sy(r[1]), r[2] * k, r[3] * k);
    }
    ctx.stroke();
    ctx.lineWidth = Math.min(8, Math.max(2.5, k / 260));
    ctx.beginPath();
    for (const b of visibleBooks) {
      const r = this.bookRect(b);
      ctx.rect(sx(r[0]), sy(r[1]), r[2] * k, r[3] * k);
    }
    ctx.stroke();

    // Verse text, where it's large enough to read. Verse numbers in rubric red.
    ctx.textBaseline = "top";
    let textShown = false;
    for (const c of visibleChapters) {
      const cr = this.chapterRect(c);
      if (Math.min(cr[2], cr[3]) * k < 60) continue;
      const [s, e] = this.chapterRange(c);
      for (let v = s; v < e; v++) {
        const r = this.verseRect(v);
        if (!visible(r)) continue;
        const { f, lines } = this.wrap(v);
        const px = f * k;
        if (px < MIN_TEXT_PX) continue;
        textShown = true;
        const x = sx(r[0]) + r[2] * k * 0.04;
        let y = sy(r[1]) + r[3] * k * 0.03;
        ctx.font = `${px}px ${this.family}`;
        ctx.fillStyle = colors.ink;
        ctx.globalAlpha = px < 8 ? 0.55 : 0.9;
        for (const line of lines) {
          ctx.fillText(line, x, y);
          y += px * LINE;
        }
        ctx.globalAlpha = 1;
        if (px > 9) {
          ctx.font = `${Math.max(10, px * 0.6)}px ${this.family}`;
          ctx.fillStyle = colors.rubric;
          ctx.textAlign = "right";
          ctx.fillText(String(v - s + 1), sx(r[0] + r[2]) - 4, sy(r[1]) + 3);
          ctx.textAlign = "left";
        }
      }
    }

    // Labels: book names in letterspaced capitals when zoomed out, chapter numbers in between.
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    const spaced = "letterSpacing" in ctx;
    for (const b of visibleBooks) {
      const r = this.bookRect(b);
      const w = r[2] * k, h = r[3] * k;
      const name = this.meta.books[b][1].toUpperCase();
      const size = Math.min(17, w / (name.length * 0.95), h * 0.3);
      if (size < 7.5 || (textShown && w > this.w * 0.6)) continue;
      ctx.font = `500 ${size}px ${this.labelFamily}`;
      if (spaced) ctx.letterSpacing = `${size * 0.16}px`;
      ctx.fillStyle = colors.paper;
      ctx.globalAlpha = 0.75;
      const tw = ctx.measureText(name).width;
      ctx.fillRect(sx(r[0]) + w / 2 - tw / 2 - 4, sy(r[1]) + h / 2 - size * 0.7, tw + 8, size * 1.4);
      ctx.globalAlpha = 1;
      ctx.fillStyle = colors.ink;
      ctx.fillText(name, sx(r[0]) + w / 2, sy(r[1]) + h / 2 + size * 0.05);
    }
    if (spaced) ctx.letterSpacing = "0px";
    if (!textShown) {
      ctx.textAlign = "left";
      ctx.textBaseline = "top";
      ctx.fillStyle = colors.quiet;
      for (const c of visibleChapters) {
        const r = this.chapterRect(c);
        const w = r[2] * k, h = r[3] * k;
        if (Math.min(w, h) < 34) continue;
        const b = this.meta.chapterBook[c];
        ctx.font = `${Math.min(15, h * 0.22)}px ${this.labelFamily}`;
        ctx.fillText(String(c - this.chapterBooks[b][0] + 1), sx(r[0]) + 5, sy(r[1]) + 4);
      }
    }
    ctx.textAlign = "left";
    ctx.textBaseline = "top";

    if (this.selected >= 0) {
      const r = this.verseRect(this.selected);
      const pad = 2.5;
      ctx.strokeStyle = colors.rubric;
      ctx.lineWidth = 1.5;
      ctx.strokeRect(sx(r[0]) - pad, sy(r[1]) - pad, Math.max(r[2] * k, 2) + pad * 2, Math.max(r[3] * k, 2) + pad * 2);
    }
  }

  bindInput() {
    const c = this.canvas;
    const pointers = new Map();
    let moved = 0;
    let pinch = null;

    c.addEventListener("pointerdown", (e) => {
      c.setPointerCapture(e.pointerId);
      pointers.set(e.pointerId, [e.offsetX, e.offsetY]);
      moved = 0;
      if (pointers.size === 2) {
        const [a, b] = [...pointers.values()];
        pinch = { d: Math.hypot(a[0] - b[0], a[1] - b[1]) };
      }
    });
    c.addEventListener("pointermove", (e) => {
      const prev = pointers.get(e.pointerId);
      if (!prev) {
        if (e.pointerType === "mouse") {
          const v = this.pick(e.offsetX, e.offsetY);
          this.onHover?.(v, e.offsetX, e.offsetY);
        }
        return;
      }
      const cur = [e.offsetX, e.offsetY];
      pointers.set(e.pointerId, cur);
      if (pointers.size === 2 && pinch) {
        const [a, b] = [...pointers.values()];
        const d = Math.hypot(a[0] - b[0], a[1] - b[1]);
        this.zoomAt((a[0] + b[0]) / 2, (a[1] + b[1]) / 2, d / pinch.d);
        pinch.d = d;
        moved += 10;
        return;
      }
      const dx = cur[0] - prev[0], dy = cur[1] - prev[1];
      moved += Math.abs(dx) + Math.abs(dy);
      this.cam.x -= dx / this.cam.k;
      this.cam.y -= dy / this.cam.k;
      this.clamp();
      this.draw();
      this.onHover?.(-1);
    });
    const end = (e) => {
      if (!pointers.has(e.pointerId)) return;
      pointers.delete(e.pointerId);
      if (pointers.size < 2) pinch = null;
      if (e.type === "pointerup" && pointers.size === 0 && moved < 6) {
        const v = this.pick(e.offsetX, e.offsetY);
        if (v >= 0) this.onPick?.(v);
      }
    };
    c.addEventListener("pointerup", end);
    c.addEventListener("pointercancel", end);
    c.addEventListener("pointerleave", () => this.onHover?.(-1));
    c.addEventListener("dblclick", (e) => this.zoomAt(e.offsetX, e.offsetY, 2.5));
    c.addEventListener("wheel", (e) => {
      e.preventDefault();
      const lines = e.deltaMode === 1;
      const mouseWheel = lines || (e.deltaX === 0 && Math.abs(e.deltaY) >= 50 && Number.isInteger(e.deltaY));
      if (e.ctrlKey || mouseWheel) {
        const dy = lines ? e.deltaY * 33 : e.deltaY;
        this.zoomAt(e.offsetX, e.offsetY, Math.exp(-dy * (e.ctrlKey ? 0.012 : 0.0025)));
      } else {
        this.cam.x += e.deltaX / this.cam.k;
        this.cam.y += e.deltaY / this.cam.k;
        this.clamp();
        this.draw();
      }
    }, { passive: false });
    c.addEventListener("keydown", (e) => {
      const step = 80 / this.cam.k;
      const keys = {
        ArrowLeft: () => (this.cam.x -= step), ArrowRight: () => (this.cam.x += step),
        ArrowUp: () => (this.cam.y -= step), ArrowDown: () => (this.cam.y += step),
        "+": () => this.zoomBy(1.6), "=": () => this.zoomBy(1.6), "-": () => this.zoomBy(1 / 1.6),
        "0": () => this.reset(),
      };
      const fn = keys[e.key];
      if (!fn) return;
      e.preventDefault();
      e.stopPropagation();
      fn();
      this.clamp();
      this.draw();
    });
    new ResizeObserver(() => this.resize()).observe(c);
  }
}
