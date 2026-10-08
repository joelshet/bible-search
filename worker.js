// Runs the Rust engine off the main thread so typing never waits on a search.
import init, { Engine } from "./pkg/bible.js";

let engine = null;
let pending = null; // newest search not yet run; older ones are dropped
let scheduled = false;

async function fetchBytes(url, onProgress) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url}: ${res.status}`);
  const total = Number(res.headers.get("content-length")) || 0;
  const reader = res.body.getReader();
  const chunks = [];
  let loaded = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
    loaded += value.length;
    onProgress?.(loaded, total);
  }
  let bytes = new Uint8Array(loaded);
  let at = 0;
  for (const c of chunks) {
    bytes.set(c, at);
    at += c.length;
  }
  // Some hosts already strip the gzip layer via Content-Encoding; only inflate real gzip.
  if (bytes[0] === 0x1f && bytes[1] === 0x8b) {
    const stream = new Blob([bytes]).stream().pipeThrough(new DecompressionStream("gzip"));
    bytes = new Uint8Array(await new Response(stream).arrayBuffer());
  }
  return bytes;
}

async function start() {
  try {
    const t0 = performance.now();
    const [, core] = await Promise.all([
      init({ module_or_path: new URL("./pkg/bible_bg.wasm", import.meta.url) }),
      fetchBytes(new URL("./data/bible.idx.gz", import.meta.url), (loaded, total) =>
        postMessage({ type: "progress", loaded, total })),
    ]);
    engine = new Engine(core);
    const meta = JSON.parse(engine.meta());
    postMessage({ type: "ready", meta, ms: performance.now() - t0 });
    if (pending) schedule();
    const lex = await fetchBytes(new URL("./data/lexicon.idx.gz", import.meta.url));
    engine.load_lexicon(lex);
    postMessage({ type: "lexicon" });
  } catch (e) {
    postMessage({ type: "error", message: String(e?.message || e) });
  }
}

function schedule() {
  if (scheduled || !engine) return;
  scheduled = true;
  // Let any keystrokes already queued arrive first, then search only the newest.
  setTimeout(runSearch, 0);
}

function runSearch() {
  scheduled = false;
  const m = pending;
  pending = null;
  if (!m || !engine) return;
  const t = performance.now();
  const summary = JSON.parse(engine.search(m.q, m.live, m.canonical));
  const ms = performance.now() - t;
  const page = JSON.parse(engine.page(0, m.limit));
  const hits = engine.hits();
  postMessage({ type: "results", id: m.id, q: m.q, summary, page, hits, ms }, [hits.buffer]);
}

const handlers = {
  page: (m) => JSON.parse(engine.page(m.offset, m.limit)),
  chapter: (m) => JSON.parse(engine.chapter(m.verse)),
  strongs: (m) => JSON.parse(engine.strongs(m.code)),
  qr: (m) => Engine.qr(m.text),
  layout: (m) => engine.layout(m.aspect),
};

onmessage = ({ data: m }) => {
  if (m.type === "search") {
    pending = m;
    schedule();
  } else if (m.type === "clear") {
    pending = null;
    engine?.clear();
  } else if (handlers[m.type]) {
    // Answer with the newest search applied, so chapters carry its highlights.
    if (pending && engine) runSearch();
    let result = null;
    let error = null;
    try {
      result = engine ? handlers[m.type](m) : null;
    } catch (e) {
      error = String(e?.message || e);
    }
    postMessage({ type: "reply", req: m.req, result, error });
  }
};

start();
