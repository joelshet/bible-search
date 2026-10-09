// Offline after the first visit: everything the app needs is cached up front.
const VERSION = "ed0ab8c63e90";
const CACHE = `bible-${VERSION}`;
const FILES = [
  "./", "index.html", "style.css", "app.js", "map.js", "reader.js", "worker.js",
  "pkg/bible.js", "pkg/bible_bg.wasm", "data/bible.idx.gz", "data/lexicon.idx.gz",
  "fonts/garamond.woff2", "fonts/garamond-italic.woff2",
  "fonts/atkinson-regular.woff2", "fonts/atkinson-bold.woff2", "fonts/atkinson-italic.woff2",
];

// build.sh stamps VERSION with a content hash; unstamped means local development, so stay out of the way.
const DEV = VERSION === "dev";

self.addEventListener("install", (e) => {
  if (DEV) return self.skipWaiting();
  e.waitUntil(saveFiles().then(() => self.skipWaiting()));
});

// Each file is saved as it arrives, so an install cut short by a closed app or a dropped
// connection carries on from where it stopped the next time.
async function saveFiles() {
  const cache = await caches.open(CACHE);
  const saved = new Set((await cache.keys()).map((r) => r.url));
  await Promise.all(FILES.map(async (f) => {
    // "no-cache" asks the server whether the browser's copy is current and downloads only when
    // it isn't. A stale copy saved under a new VERSION would be served from then on.
    const req = new Request(f, { cache: "no-cache" });
    if (saved.has(req.url)) return;
    const res = await fetch(req);
    if (!res.ok) throw new Error(`${f}: ${res.status}`);
    await cache.put(req, res);
  }));
}

// Other apps can share this origin and keep caches of their own, so only this app's old
// ones go. Before the "bible-" prefix a cache was named by its bare stamp.
const stale = (k) => k !== CACHE && (k.startsWith("bible-") || /^[0-9a-f]{12}$/.test(k));

self.addEventListener("activate", (e) => {
  e.waitUntil(
    caches.keys()
      .then((keys) => Promise.all(keys.filter(stale).map((k) => caches.delete(k))))
      .then(() => self.clients.claim()),
  );
});

self.addEventListener("fetch", (e) => {
  if (DEV || e.request.method !== "GET" || new URL(e.request.url).origin !== location.origin) return;
  // Pages carry the query in ?q=..., so match them against the cached shell.
  const page = e.request.mode === "navigate";
  e.respondWith(
    caches.match(e.request, { ignoreSearch: page }).then((hit) => hit || fetch(e.request)),
  );
});
