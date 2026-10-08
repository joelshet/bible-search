import { BibleMap } from "./map.js";
import { Reader } from "./reader.js";

const $ = (id) => document.getElementById(id);
const root = document.documentElement;
const shell = document.querySelector("bible-search");
const input = $("q");
const list = $("list");
const status = $("status");
const tryLine = status.firstElementChild; // the example searches, written in index.html
const PAGE = 40;

const worker = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
let meta = null;
let map = null;
let searchId = 0;
let shown = { id: 0, total: 0, loaded: 0, q: "", summary: null, ms: 0 };
let selected = -1;          // index into the result list
let openVerse = -1;         // verse whose chapter is expanded
let focusVerse = -1;        // verse id the URL, map, QR, and present mode refer to
let requests = new Map();
let reader = null;
let mode = "search";        // or "read"
let listScroll = 0;         // where the results were scrolled to when reading began
let reqId = 0;

// ---------- settings (remembered per browser) ----------

const narrow = matchMedia("(max-width: 860px)");
// mapRead: null means "hide while reading on a phone, show on a laptop".
const defaults = { size: "m", spacing: "normal", font: "serif", theme: "auto", ruler: false, italics: true, red: false, paragraphs: false, motion: false, bottom: false, rate: 1, sort: "relevance", map: true, mapRead: null };
let settings = { ...defaults };
try {
  Object.assign(settings, JSON.parse(localStorage.getItem("bible-settings") || "{}"));
} catch {}

function applySettings() {
  root.dataset.size = settings.size;
  root.dataset.spacing = settings.spacing;
  root.dataset.font = settings.font;
  root.dataset.theme = settings.theme;
  root.dataset.ruler = settings.ruler ? "on" : "off";
  root.dataset.italics = settings.italics ? "on" : "off";
  root.dataset.red = settings.red ? "on" : "off";
  root.dataset.paragraphs = settings.paragraphs ? "on" : "off";
  root.dataset.bottom = settings.bottom ? "on" : "off";
  fitKeyboard();
  const showMap = mapShown();
  shell.dataset.map = showMap ? "shown" : "hidden";
  $("map-btn").setAttribute("aria-pressed", String(showMap));
  $("sort-btn").textContent = settings.sort === "canonical" ? "bible order" : "relevance";
  try {
    localStorage.setItem("bible-settings", JSON.stringify(settings));
  } catch {}
  if (map) {
    map.setColors(mapColors());
    map.setFont(getComputedStyle(root).getPropertyValue("--read"));
  }
}

function mapShown() {
  if (mode !== "read") return settings.map;
  return settings.mapRead ?? !narrow.matches;
}

function mapColors() {
  const css = getComputedStyle(root);
  const v = (name) => css.getPropertyValue(name).trim();
  return {
    paper: v("--paper"), ink: v("--ink"), quiet: v("--quiet"), rubric: v("--rubric"),
    tile: v("--tile"), tileAlt: v("--tile-alt"), glow: v("--glow"),
    dark: css.colorScheme === "dark" || css.getPropertyValue("color-scheme").trim() === "dark",
  };
}

// A search box at the bottom sits where the phone keyboard opens. While that setting is on,
// the app is sized to the part of the screen the keyboard leaves, and the map gives its
// band to the results while the keyboard is up.
const vv = window.visualViewport;
let fullHeight = 0;
let fullWidth = 0;
function fitKeyboard() {
  if (!vv || !settings.bottom || !narrow.matches || Math.abs(vv.scale - 1) > 0.01) {
    root.style.removeProperty("--app-height");
    shell.removeAttribute("data-keyboard");
    return;
  }
  if (vv.width !== fullWidth) fullHeight = 0; // the phone was turned
  fullWidth = vv.width;
  fullHeight = Math.max(fullHeight, vv.height);
  root.style.setProperty("--app-height", `${vv.height}px`);
  shell.toggleAttribute("data-keyboard", vv.height < fullHeight - 120);
  scrollTo(0, 0);
}
vv?.addEventListener("resize", fitKeyboard);
vv?.addEventListener("scroll", fitKeyboard);

// ---------- worker plumbing ----------

function ask(type, data) {
  const req = ++reqId;
  worker.postMessage({ type, req, ...data });
  return new Promise((resolve, reject) => requests.set(req, { resolve, reject }));
}

worker.onmessage = ({ data: m }) => {
  if (m.type === "progress") {
    const mb = (n) => (n / 1e6).toFixed(1);
    $("loading").textContent = m.total ? `Loading the Bible… ${mb(m.loaded)} of ${mb(m.total)} MB` : "Loading the Bible…";
  } else if (m.type === "ready") {
    ready(m);
  } else if (m.type === "results") {
    showResults(m);
  } else if (m.type === "reply") {
    const r = requests.get(m.req);
    requests.delete(m.req);
    if (r) m.error ? r.reject(new Error(m.error)) : r.resolve(m.result);
  } else if (m.type === "error") {
    $("loading").textContent = `Couldn't load: ${m.message}`;
  }
};

function ready(m) {
  meta = m.meta;
  meta.text = meta.text.split("\n");
  meta.chapterNum = [];
  let last = -1, n = 0;
  meta.chapterBook.forEach((b) => {
    n = b === last ? n + 1 : 1;
    last = b;
    meta.chapterNum.push(n);
  });
  $("loading").textContent = "";
  new ResizeObserver(fitLayout).observe($("map-panel"));
  reader = new Reader($("reader"), $("results"), {
    ask,
    meta,
    verseHtml: versePlain,
    onVerse: (v, explicit) => {
      focusVerse = v;
      syncRail();
      map?.setSelected(v);
      try {
        localStorage.setItem("bible-last", String(v));
      } catch {}
      syncUrl(false);
      if (explicit && settings.motion) map?.locate(v);
    },
  });
  $("books").innerHTML = reader.booksHtml();
  applySettings();
  restoreFromUrl();
  // Canvas text can't use a web font until it has loaded; redraw once it has.
  document.fonts.load('20px "EB Garamond"').then(() => map?.remeasure());
}

// The map is laid out for the shape of its panel: a wide band on a phone, the side panel on a laptop.
// It only re-lays out when that shape changes a lot, so the geography stays put.
let layoutAspect = 0;
let layoutBusy = false;
async function fitLayout() {
  const { width, height } = $("map-panel").getBoundingClientRect();
  if (!meta || !width || !height) return;
  clearMapTools();
  if (layoutBusy) return;
  const aspect = Math.min(3, Math.max(0.5, width / height));
  if (layoutAspect && Math.abs(Math.log(aspect / layoutAspect)) < 0.2) return;
  layoutBusy = true;
  const layout = await ask("layout", { aspect });
  layoutBusy = false;
  layoutAspect = aspect;
  if (map) {
    map.setLayout(layout, aspect);
    return;
  }
  map = new BibleMap($("map"), {
    layout,
    aspect,
    meta,
    onPick: (v) => openReader(v),
    onHover: (v, x, y) => {
      const tip = $("tip");
      if (v < 0) return void (tip.hidden = true);
      tip.textContent = refOf(v);
      tip.style.left = x + "px";
      tip.style.top = y + "px";
      tip.hidden = false;
    },
  });
  map.setColors(mapColors());
  map.setFont(getComputedStyle(root).getPropertyValue("--read"));
  map.setHits(lastHits);
  map.setSelected(focusVerse);
  clearMapTools();
}

// Book labels stay out from under the zoom buttons.
function clearMapTools() {
  if (!map) return;
  const panel = $("map").getBoundingClientRect();
  const tools = document.querySelector(".map-tools").getBoundingClientRect();
  map.keepClear = [[tools.left - panel.left - 6, tools.top - panel.top, tools.right - panel.left, tools.bottom - panel.top]];
  map.draw();
}

// ---------- verse ids ----------

function chapterOf(v) {
  const s = meta.chapterStart;
  let lo = 0, hi = s.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (s[mid] <= v) lo = mid; else hi = mid - 1;
  }
  return lo;
}
function refParts(v) {
  const c = chapterOf(v);
  return { book: meta.chapterBook[c], chapter: meta.chapterNum[c], verse: v - meta.chapterStart[c] + 1 };
}
function refOf(v) {
  const p = refParts(v);
  return `${meta.books[p.book][1]} ${p.chapter}:${p.verse}`;
}
function urlRef(v) {
  const p = refParts(v);
  return `${meta.books[p.book][0]}.${p.chapter}.${p.verse}`;
}
function parseUrlRef(s) {
  const [code, c, vs] = (s || "").split(".");
  const b = meta.books.findIndex((x) => x[0] === code);
  if (b < 0) return -1;
  for (let i = 0; i < meta.chapterStart.length; i++) {
    if (meta.chapterBook[i] === b && meta.chapterNum[i] === Number(c)) {
      const v = meta.chapterStart[i] + Math.max(1, Number(vs) || 1) - 1;
      const end = i + 1 < meta.chapterStart.length ? meta.chapterStart[i + 1] : meta.text.length;
      return v < end ? v : -1;
    }
  }
  return -1;
}

// ---------- URL state ----------

function stateUrl() {
  const p = new URLSearchParams();
  if (input.value.trim()) p.set("q", input.value.trim());
  if (mode === "read") p.set("read", focusVerse >= 0 ? urlRef(focusVerse) : "");
  else if (focusVerse >= 0) p.set("v", urlRef(focusVerse));
  const qs = p.toString().replace(/%3A/g, ":").replace(/%2C/g, ",");
  return location.pathname + (qs ? "?" + qs : "");
}
let lastPushed = "";
function syncUrl(push) {
  const url = stateUrl();
  if (url === location.pathname + location.search) return;
  // Browsers refuse history updates that come too fast, and some throw; a long scroll through
  // the reader can get there. The address then lags until the next update goes through.
  try {
    if (push && url !== lastPushed) {
      history.pushState(null, "", url);
      lastPushed = url;
    } else history.replaceState(null, "", url);
  } catch {}
}
let pauseTimer = 0;

function restoreFromUrl() {
  const p = new URLSearchParams(location.search);
  const q = p.get("q") || "";
  // Results still on the page stay as they are, so Back returns to the same hit,
  // scroll position, and open chapter.
  const kept = q !== "" && q === shown.q.trim();
  input.value = q;
  if (p.has("read")) {
    const v = parseUrlRef(p.get("read"));
    if (!kept) runSearch(false);
    if (v >= 0) openReader(v, false);
    else openContents(false);
    return;
  }
  setMode("search");
  focusVerse = parseUrlRef(p.get("v"));
  if (kept) {
    const i = [...items()].findIndex((el) => Number(el.dataset.v) === focusVerse);
    if (i >= 0) select(i, false);
    else map?.setSelected(focusVerse);
    return;
  }
  pendingOpen = input.value ? focusVerse : -1;
  runSearch(false);
  if (!input.value && focusVerse >= 0) openReader(focusVerse, false);
}
window.addEventListener("popstate", () => {
  lastPushed = ""; // after Back, opening the same verse again must add a step, not replace this one
  if (meta) restoreFromUrl();
});

// ---------- reading ----------

function setMode(m) {
  const changed = m !== mode;
  // Results and the reader scroll in the same box, so the results' place is kept by hand.
  if (changed && m === "read") listScroll = $("results").scrollTop;
  if (changed && m !== "read" && speaking >= 0) stopListening();
  mode = m;
  shell.dataset.mode = m;
  if (changed) applySettings(); // only the map's visibility depends on the mode
  if (changed && m === "search") $("results").scrollTop = listScroll;
  syncRail();
  if (changed) showPlace();
}

function openReader(v, push = true) {
  if (!reader || v < 0) return;
  setMode("read");
  if (speaking >= 0) return speakFrom(v, push); // the voice goes where you go
  focusVerse = v;
  syncRail();
  map?.setSelected(v);
  reader.open(v);
  syncUrl(push);
}

// Beside the text, where the screen has room: every chapter of the book being read.
let railBook = -1;
function syncRail() {
  const rail = $("rail");
  const c = meta && mode === "read" && focusVerse >= 0 ? chapterOf(focusVerse) : -1;
  const b = c < 0 ? -1 : meta.chapterBook[c];
  if (b !== railBook) {
    railBook = b;
    const chapters = b < 0 ? [] : meta.chapterBook.flatMap((book, i) => (book === b ? [i] : []));
    rail.innerHTML = chapters.length > 1 ? chapters.map((i) => reader.chapterLink(i, meta.chapterNum[i])).join("") : "";
  }
  rail.hidden = !rail.firstChild;
  const now = rail.children[c < 0 ? -1 : meta.chapterNum[c] - 1];
  if (!now || now.hasAttribute("aria-current")) return;
  rail.querySelector("[aria-current]")?.removeAttribute("aria-current");
  now.setAttribute("aria-current", "true");
  now.scrollIntoView({ block: "nearest" });
}

function openContents(push = true) {
  if (!reader) return;
  if (speaking >= 0) stopListening();
  setMode("read");
  focusVerse = -1;
  syncRail();
  map?.setSelected(-1);
  reader.contents();
  syncUrl(push);
}

function lastRead() {
  try {
    const v = Number(localStorage.getItem("bible-last"));
    return Number.isInteger(v) && v >= 0 && v < meta.text.length ? v : -1;
  } catch {
    return -1;
  }
}

$("read-btn").onclick = () => {
  if (mode === "read") return leaveReader();
  const v = currentVerse() >= 0 ? currentVerse() : lastRead();
  v >= 0 ? openReader(v) : openContents();
};

// Back to the results, or to the home view when there was no search.
function leaveReader() {
  setMode("search");
  focusVerse = -1;
  syncUrl(true);
}

// A click on a chapter link or button opens it here. With a modifier key the link's
// own address opens in a new tab or window instead.
function followGo(e) {
  const go = e.target.closest("[data-go]");
  if (!go || !reader || e.metaKey || e.ctrlKey || e.shiftKey) return false;
  e.preventDefault();
  openReader(Number(go.dataset.go));
  return true;
}
$("books").addEventListener("click", followGo);
$("rail").addEventListener("click", followGo);

// The title is the way home: no search, the map, and the list of books.
$("title").addEventListener("click", (e) => {
  if (!meta || e.metaKey || e.ctrlKey || e.shiftKey) return;
  e.preventDefault();
  input.value = "";
  focusVerse = -1;
  setMode("search");
  runSearch(false);
  syncUrl(true);
});

$("reader").addEventListener("click", (e) => {
  const t = e.target;
  if (followGo(e)) return;
  if (t.closest('[data-act="contents"]')) return openContents();
  const word = t.closest(".w[data-s]");
  if (word) return openLexicon(word.dataset.s);
  const line = t.closest(".vl");
  if (line && speaking >= 0) return speakFrom(Number(line.dataset.v));
  if (line) {
    reader.mark(Number(line.dataset.v));
    focusVerse = Number(line.dataset.v);
    map?.setSelected(focusVerse);
    syncUrl(false);
  }
});

// ---------- searching ----------

let pendingOpen = -1;
let lastHits = null;

function runSearch(live) {
  const q = input.value;
  shell.toggleAttribute("data-has-query", q.trim().length > 0);
  if (!q.trim()) {
    worker.postMessage({ type: "clear" });
    searchId++;
    shown = { id: searchId, total: 0, loaded: 0, q: "", summary: null, ms: 0 };
    if (pendingOpen < 0) list.innerHTML = "";
    resetListScroll();
    showPlace();
    selected = -1;
    lastHits = null;
    map?.setHits(null);
    return;
  }
  worker.postMessage({ type: "search", id: ++searchId, q, live, canonical: settings.sort === "canonical", limit: PAGE });
}

// Clear the search without leaving the page you're on. The reader keeps its place and
// loses its underlines; the results give way to the home view.
function clearSearch() {
  input.value = "";
  if (mode !== "read") focusVerse = -1;
  runSearch(false);
  for (const el of $("reader").querySelectorAll(".m")) el.className = el.className.replace(/ m( m-\w+)?/, "");
  syncUrl(true);
}
$("clear").onclick = () => {
  clearSearch();
  if (mode !== "read") input.focus(); // while reading, focus would bring the keyboard up over the page
};

input.addEventListener("input", () => {
  if (!meta) return;
  setMode("search");
  focusVerse = -1;
  openVerse = -1;
  runSearch(true);
  syncUrl(false);
  clearTimeout(pauseTimer);
  // A pause in typing becomes a back-button step.
  pauseTimer = setTimeout(() => syncUrl(true), 1500);
});
$("search-form").addEventListener("submit", (e) => {
  e.preventDefault();
  if (selected < 0 && shown.total) select(0);
  syncUrl(true);
});

// New results start at the top, including ones that arrive while the reader is showing.
function resetListScroll() {
  if (mode === "search") $("results").scrollTop = 0;
  else listScroll = 0;
}

function fmtTime(ms) {
  if (ms <= 0) return "under 1 ms";
  if (ms < 1) return `${Math.max(1, Math.round(ms * 1000))} µs`;
  return `${ms.toFixed(1)} ms`;
}
const plural = (n, word) => `${n.toLocaleString()} ${word}${n === 1 ? "" : "s"}`;

// The controls that say where you are. The line under the search box holds examples to try
// while the box is empty, the way back to the results while reading, and otherwise what
// the search found.
function showPlace() {
  const searched = shown.q.trim() !== "";
  $("read-btn").setAttribute("aria-pressed", String(mode === "read"));
  $("sort-btn").disabled = !searched || mode === "read";
  $("listen-btn").setAttribute("aria-pressed", String(speaking >= 0));
  // Nothing to say above a chapter when there's no search and nothing playing.
  status.hidden = !searched && mode === "read" && speaking < 0;
  status.scrollLeft = 0;
  if (!searched && speaking < 0) return status.replaceChildren(tryLine);
  status.replaceChildren();
  const add = (html, cls) => {
    const span = document.createElement("span");
    if (cls) span.className = cls;
    span.innerHTML = html;
    status.append(span);
    return span;
  };
  if (speaking >= 0) {
    add(`<button type="button" data-listen="stop">stop</button>`);
    add(`<button type="button" data-listen="slower">slower</button> ${settings.rate}× <button type="button" data-listen="faster">faster</button>`);
  }
  if (mode === "read") {
    if (!searched) return;
    const back = add(`<button type="button">← ${plural(shown.total, "result")} for “${esc(shown.q.trim())}”</button>`);
    back.querySelector("button").onclick = leaveReader;
    return;
  }
  const s = shown.summary;
  if (s.reference) add(s.total ? plural(s.total, "verse") : "No such passage");
  else if (!s.total) add("Nothing matched. Try fewer words, or say it the way you remember it.");
  else if (s.terms > 1) add(`${s.full.toLocaleString()} with every word · ${plural(s.total, "verse")} with some · ${plural(s.books, "book")}`);
  else if (s.literal && s.literal < s.total) add(`${plural(s.literal, "verse")} with “${esc(s.word)}” · ${s.total.toLocaleString()} counting related words · ${plural(s.books, "book")}`);
  else add(`${plural(s.total, "verse")} · ${plural(s.books, "book")}`);
  for (const n of s.notes) add(`spelling: ${esc(n)}`, "note");
  if (s.book) {
    const b = add(`<button type="button">Open ${esc(s.book.name)} 1</button>`);
    b.querySelector("button").onclick = () => openReader(s.book.first);
  }
  add(`search ${fmtTime(shown.ms)}`);
}

function showResults(m) {
  if (m.id !== searchId) return;
  shown = { id: m.id, total: m.summary.total, loaded: m.page.length, q: m.q, summary: m.summary, ms: m.ms };
  showPlace();
  list.innerHTML = m.page.map(hitHtml).join("");
  resetListScroll();
  selected = -1;
  openVerse = -1;
  lastHits = m.hits;
  map?.setHits(m.hits);
  if (pendingOpen >= 0) {
    const v = pendingOpen;
    pendingOpen = -1;
    const i = m.page.findIndex((h) => h.id === v);
    if (i >= 0) {
      select(i, false);
      toggleContext(i);
    } else if (mode === "search") openReader(v, false);
  }
}

async function loadMore() {
  if (!shown.q || shown.loaded >= shown.total || loadMore.busy) return;
  loadMore.busy = true;
  const id = shown.id;
  const items = await ask("page", { offset: shown.loaded, limit: PAGE });
  loadMore.busy = false;
  if (id !== shown.id) return;
  shown.loaded += items.length;
  list.insertAdjacentHTML("beforeend", items.map(hitHtml).join(""));
}
new IntersectionObserver((e) => e[0].isIntersecting && loadMore(), { root: $("results"), rootMargin: "600px" }).observe($("more"));

// ---------- rendering verses ----------

function esc(s) {
  return String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
}

const pilcrow = (html) => html.replace("¶", '<span class="pilcrow">¶</span>');
function segHtml(seg) {
  if (seg.length === 1) return pilcrow(esc(seg[0]));
  const [text, flags, why, strongs] = seg;
  const cls = ["w"];
  if (flags & 1) cls.push("it");
  if (flags & 2) cls.push("wj");
  if (why) cls.push("m", "m-" + why);
  const s = strongs ? ` data-s="${strongs.split(" ")[0]}"` : "";
  return `<span class="${cls.join(" ")}"${s}>${esc(text)}</span>`;
}
const versePlain = (segs) => segs.map(segHtml).join("");

// A hit reads top to bottom: where it is and what you can do there, the verse, then why it matched.
function hitHtml(h) {
  const why = (h.why || []).map(([text, code]) => code
    ? `<button type="button" class="why" data-code="${code}">${esc(text)}</button>`
    : `<span class="why">${esc(text)}</span>`).join("");
  return `<li class="hit${h.full ? "" : " partial"}" data-v="${h.id}">
    <div class="hit-head"><button type="button" class="ref">${esc(h.ref)}</button>
      <button type="button" class="act" data-act="context" aria-expanded="false">context</button>
      <button type="button" class="act" data-read="${h.id}">read</button></div>
    <p class="text" data-v="${h.id}"><sup>${h.v}</sup>${versePlain(h.seg)}</p>${why ? `<p class="notes">${why}</p>` : ""}</li>`;
}

function items() {
  return list.querySelectorAll(".hit");
}

function select(i, scroll = true) {
  const all = items();
  if (!all.length) return;
  i = Math.max(0, Math.min(i, all.length - 1));
  all[selected]?.classList.remove("selected");
  selected = i;
  const el = all[i];
  el.classList.add("selected");
  focusVerse = Number(el.dataset.v);
  map?.setSelected(focusVerse);
  if (scroll) el.scrollIntoView({ block: "nearest", behavior: settings.motion ? "smooth" : "auto" });
  syncUrl(false);
  if (i >= all.length - 5) loadMore();
}

const AROUND = 3; // verses of context shown on each side of a hit, and added by "more"

async function toggleContext(i) {
  const el = items()[i];
  if (!el) return;
  const button = el.querySelector('[data-act="context"]');
  if (el.classList.contains("open")) {
    for (const box of el.querySelectorAll(".context")) box.remove();
    el.querySelector(".here")?.classList.remove("here");
    el.classList.remove("open");
    button.setAttribute("aria-expanded", "false");
    openVerse = -1;
    return;
  }
  openVerse = Number(el.dataset.v);
  el.classList.add("open");
  el.querySelector(".text").classList.add("here");
  button.setAttribute("aria-expanded", "true");
  await renderContext(el, AROUND);
  syncUrl(true);
}

// The verses around a hit, `around` on each side and never past its chapter. They go above
// and below the hit's own text, so the verse you opened is still the one in the middle.
async function renderContext(el, around) {
  const v = Number(el.dataset.v);
  const ch = await ask("chapter", { verse: v });
  if (!ch || !el.classList.contains("open")) return;
  for (const box of el.querySelectorAll(".context")) box.remove();
  const at = ch.verses.findIndex((x) => x.id === v);
  const before = ch.verses.slice(Math.max(0, at - around), at);
  const after = ch.verses.slice(at + 1, at + 1 + around);
  const html = (verses) => verses.map((x) => `<p data-v="${x.id}"><sup>${x.v}</sup>${versePlain(x.seg)}</p>`).join("");
  const more = before.length + after.length + 1 < ch.verses.length
    ? `<nav><button type="button" data-more="${around + AROUND}">more</button></nav>` : "";
  const text = el.querySelector(".text");
  if (before.length) text.insertAdjacentHTML("beforebegin", `<div class="context">${html(before)}</div>`);
  if (after.length || more) text.insertAdjacentHTML("afterend", `<div class="context">${html(after)}${more}</div>`);
  text.scrollIntoView({ block: "nearest" });
}

list.addEventListener("click", (e) => {
  const t = e.target;
  const hit = t.closest(".hit");
  if (!hit) return;
  const i = [...items()].indexOf(hit);
  if (t.closest("[data-read]")) {
    select(i, false); // so Back from the reader lands on this hit
    return openReader(Number(t.closest("[data-read]").dataset.read));
  }
  if (t.closest("[data-more]")) return renderContext(hit, Number(t.closest("[data-more]").dataset.more));
  if (t.closest('.ref, [data-act="context"]')) {
    select(i, false);
    toggleContext(i);
    return;
  }
  const word = t.closest(".w[data-s], .why[data-code]");
  if (word) {
    openLexicon(word.dataset.s || word.dataset.code);
    return;
  }
  const p = t.closest(".open p[data-v]");
  if (p) {
    hit.querySelector(".here")?.classList.remove("here");
    p.classList.add("here");
    focusVerse = Number(p.dataset.v);
    map?.setSelected(focusVerse);
    syncUrl(false);
    return;
  }
  if (i !== selected) select(i, false);
});

// ---------- word study ----------

async function openLexicon(code) {
  const e = await ask("strongs", { code });
  const panel = $("lex");
  const body = $("lex-body");
  if (!e) {
    body.innerHTML = `<p>The dictionary is still loading. Try again in a moment.</p>`;
    panel.hidden = false;
    return;
  }
  const linkCodes = (s) => esc(s).replace(/\b([HG])0*(\d{1,4})\b/g, '<a href="#" class="code-link" data-code="$1$2">$1$2</a>');
  const lang = e.lang === "hebrew" ? "he" : "grc";
  body.innerHTML = `
    <div class="code">${esc(e.code)} · ${e.lang === "hebrew" ? "Hebrew" : "Greek"}</div>
    <div class="lemma" lang="${lang}" dir="${lang === "he" ? "rtl" : "ltr"}">${esc(e.lemma)}</div>
    <div><span class="translit">${esc(e.translit)}</span> ${e.pron ? `<span class="pron">(${esc(e.pron)})</span>` : ""}</div>
    <p class="def">${esc(e.definition.trim())}</p>
    ${e.derivation ? `<p class="deriv">${linkCodes(e.derivation)}</p>` : ""}
    <h4>How the KJV translates it</h4>
    <ul class="renderings">${e.renderings.map(([w, n]) => `<li>${esc(w)}<b>${n}</b></li>`).join("")}</ul>
    <p class="usage">${plural(e.uses, "use")} in ${plural(e.verses, "verse")}. Strong's KJV usage: ${esc(e.usage)}</p>
    <div class="actions"><button type="button" data-search="${esc(e.code)}">Search every verse with ${esc(e.code)}</button></div>`;
  panel.hidden = false;
}
$("lex").addEventListener("click", (e) => {
  const link = e.target.closest("[data-code]");
  if (link) {
    e.preventDefault();
    openLexicon(link.dataset.code);
  }
  const s = e.target.closest("[data-search]");
  if (s) {
    input.value = s.dataset.search;
    focusVerse = -1;
    runSearch(false);
    syncUrl(true);
  }
  if (e.target.closest(".close")) $("lex").hidden = true;
});

// ---------- listening ----------

// The device's own voice reads on from the current verse, one verse per utterance, so the
// page can follow and a tap on any verse jumps there at once.
const synth = window.speechSynthesis;
const RATES = [0.8, 1, 1.25, 1.5, 2];
let speaking = -1; // the verse being read aloud, or -1
let speakTurn = 0; // changes on every start and stop, so a cancelled verse's ending is ignored
let utterance = null; // held here because some browsers drop an utterance nobody references
let wake = null;

function sayable(v) {
  const c = chapterOf(v);
  const heading = v === meta.chapterStart[c] ? `${meta.books[meta.chapterBook[c]][1]} ${meta.chapterNum[c]}. ` : "";
  // The KJV prints LORD and GOD in capitals, which a voice may spell out letter by letter.
  return heading + meta.text[v].replace("¶", "").trim().replace(/\b[A-Z]{2,}\b/g, (w) => w[0] + w.slice(1).toLowerCase());
}

function speakFrom(v, push = false) {
  const turn = ++speakTurn;
  if (synth.speaking || synth.pending) synth.cancel();
  if (v < 0 || v >= meta.text.length) return stopListening();
  speaking = v;
  utterance = new SpeechSynthesisUtterance(sayable(v));
  utterance.rate = settings.rate;
  utterance.onend = () => turn === speakTurn && speakFrom(v + 1);
  utterance.onerror = () => turn === speakTurn && stopListening();
  synth.speak(utterance);

  focusVerse = v;
  syncRail();
  map?.setSelected(v);
  const line = $("reader").querySelector(`.vl[data-v="${v}"]`);
  if (line) {
    reader.mark(v);
    line.scrollIntoView({ block: "center", behavior: settings.motion ? "smooth" : "auto" });
  } else reader.open(v);
  syncUrl(push);
}

function stopListening() {
  speakTurn++;
  speaking = -1;
  synth.cancel();
  wake?.release();
  wake = null;
  showPlace();
}

$("listen-btn").onclick = () => {
  if (speaking >= 0) return stopListening();
  if (!meta) return;
  let v = currentVerse();
  if (v < 0) v = lastRead();
  if (mode !== "read") setMode("read");
  speakFrom(Math.max(v, 0), true);
  showPlace();
  // A phone that dims its screen stops the voice, so keep it awake while listening.
  navigator.wakeLock?.request("screen").then((lock) => (speaking >= 0 ? (wake = lock) : lock.release()), () => {});
};
$("listen-btn").hidden = !synth;

status.addEventListener("click", (e) => {
  const act = e.target.closest("[data-listen]")?.dataset.listen;
  if (!act || speaking < 0) return;
  if (act === "stop") return stopListening();
  const i = Math.max(0, RATES.indexOf(settings.rate)) + (act === "faster" ? 1 : -1);
  settings.rate = RATES[Math.min(Math.max(i, 0), RATES.length - 1)];
  applySettings();
  speakFrom(speaking); // a new speed takes effect from the start of the verse
  showPlace();
});
// A voice left running would carry on over the next page.
addEventListener("pagehide", () => synth?.cancel());

// ---------- present, QR, settings, help ----------

function currentVerse() {
  if (focusVerse >= 0) return focusVerse;
  const el = items()[Math.max(selected, 0)];
  return el ? Number(el.dataset.v) : -1;
}

function fitPresent() {
  const text = $("present-text");
  const fig = text.parentElement;
  let lo = 16, hi = 140;
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1;
    text.style.fontSize = mid + "px";
    if (fig.scrollHeight <= fig.clientHeight && fig.scrollWidth <= fig.clientWidth) lo = mid; else hi = mid;
  }
  text.style.fontSize = lo + "px";
}
function showPresent(v) {
  if (v < 0 || !meta) return;
  focusVerse = v;
  $("present-text").textContent = meta.text[v];
  $("present-ref").textContent = refOf(v) + " KJV";
  const d = $("present-dialog");
  if (!d.open) d.showModal();
  fitPresent();
  map?.setSelected(v);
  syncUrl(false);
}
$("present-btn").onclick = () => showPresent(currentVerse() >= 0 ? currentVerse() : 0);
$("present-dialog").addEventListener("keydown", (e) => {
  const v = focusVerse;
  if (e.key === "ArrowRight" || e.key === " " || e.key === "PageDown") showPresent(Math.min(v + 1, meta.text.length - 1));
  else if (e.key === "ArrowLeft" || e.key === "PageUp") showPresent(Math.max(v - 1, 0));
  else if (e.key === "f") document.fullscreenElement ? document.exitFullscreen() : $("present-dialog").requestFullscreen?.();
  else return;
  e.preventDefault();
});
$("present-dialog").addEventListener("click", (e) => {
  const half = e.clientX > window.innerWidth / 2;
  showPresent(Math.max(0, Math.min(focusVerse + (half ? 1 : -1), meta.text.length - 1)));
});
window.addEventListener("resize", () => $("present-dialog").open && fitPresent());

async function showQr() {
  if (!meta) return;
  const url = location.origin + stateUrl();
  $("qr-code").innerHTML = await ask("qr", { text: url });
  const v = currentVerse();
  $("qr-label").textContent = focusVerse >= 0 ? refOf(focusVerse) : input.value ? `“${input.value}”` : "Bible Search";
  $("qr-url").textContent = url;
  $("qr-dialog").showModal();
  if (v >= 0) syncUrl(true);
}
$("qr-btn").onclick = showQr;

function openSettings() {
  const f = $("settings-form");
  for (const [k, v] of Object.entries(settings)) {
    const els = f.elements[k];
    if (!els) continue;
    if (els.type === "checkbox") els.checked = !!v;
    else if (els.length) for (const r of els) r.checked = r.value === v;
  }
  $("settings-dialog").showModal();
}
$("settings-form").addEventListener("change", (e) => {
  const t = e.target;
  settings[t.name] = t.type === "checkbox" ? t.checked : t.value;
  applySettings();
});
$("settings-btn").onclick = openSettings;
$("help-btn").onclick = () => $("help-dialog").showModal();
$("sort-btn").onclick = () => {
  settings.sort = settings.sort === "canonical" ? "relevance" : "canonical";
  applySettings();
  runSearch(false);
};
$("map-btn").onclick = () => {
  if (mode === "read") settings.mapRead = !mapShown();
  else settings.map = !settings.map;
  applySettings();
};
narrow.addEventListener("change", applySettings);
$("zoom-in").onclick = () => map?.zoomBy(1.8);
$("zoom-out").onclick = () => map?.zoomBy(1 / 1.8);
$("zoom-reset").onclick = () => map?.reset();
$("zoom-locate").onclick = () => map?.locate(currentVerse());
document.querySelectorAll("[data-q]").forEach((b) =>
  b.addEventListener("click", () => {
    input.value = b.dataset.q;
    focusVerse = -1;
    runSearch(false);
    syncUrl(true);
    input.focus();
  }));
matchMedia("(prefers-color-scheme: dark)").addEventListener("change", applySettings);

// ---------- keyboard ----------

document.addEventListener("keydown", (e) => {
  if (e.metaKey || e.ctrlKey || e.altKey) return;
  if (document.querySelector("dialog[open]")) return; // dialogs handle their own keys; Esc closes them
  const inInput = e.target === input;
  const inField = inInput || e.target.closest?.("input, textarea, select");
  const k = e.key;

  if (inInput) {
    if (k === "ArrowDown") { e.preventDefault(); select(selected + 1); }
    else if (k === "ArrowUp") { e.preventDefault(); select(selected - 1); }
    else if (k === "Enter" && selected >= 0) { e.preventDefault(); toggleContext(selected); }
    else if (k === "Escape") {
      e.preventDefault();
      if (!$("lex").hidden) $("lex").hidden = true;
      else if (input.value) clearSearch();
      else input.blur();
    }
    return;
  }
  if (inField) return;

  if (mode === "read") {
    const from = focusVerse >= 0 ? focusVerse : 0;
    const readKeys = {
      ArrowRight: () => openReader(reader.chapterStep(1, from)), "]": () => openReader(reader.chapterStep(1, from)),
      ArrowLeft: () => openReader(reader.chapterStep(-1, from)), "[": () => openReader(reader.chapterStep(-1, from)),
      j: () => reader.step(1), ArrowDown: () => reader.step(1),
      k: () => reader.step(-1), ArrowUp: () => reader.step(-1),
      c: () => openContents(),
      Escape: () => (!$("lex").hidden ? ($("lex").hidden = true) : leaveReader()),
    };
    if (readKeys[k]) {
      e.preventDefault();
      readKeys[k]();
      return;
    }
  }

  const actions = {
    "/": () => { input.focus(); input.select(); },
    j: () => select(selected + 1), ArrowDown: () => select(selected + 1),
    k: () => select(selected - 1), ArrowUp: () => select(selected - 1),
    Enter: () => selected >= 0 && toggleContext(selected),
    o: () => selected >= 0 && toggleContext(selected),
    p: () => showPresent(currentVerse()),
    q: showQr,
    l: () => map?.locate(currentVerse()),
    r: () => $("read-btn").click(),
    c: () => openContents(),
    m: () => $("map-btn").click(),
    s: () => $("sort-btn").click(),
    ",": openSettings,
    "?": () => $("help-dialog").showModal(),
    Escape: () => {
      if (!$("lex").hidden) $("lex").hidden = true;
      else if (openVerse >= 0 && selected >= 0) toggleContext(selected);
    },
  };
  const fn = actions[k];
  if (!fn) return;
  e.preventDefault();
  fn();
});

// ---------- start ----------

applySettings();
if ("serviceWorker" in navigator && location.protocol !== "file:") {
  navigator.serviceWorker.register("sw.js").catch(() => {});
}
if (!matchMedia("(pointer: coarse)").matches) input.focus();
