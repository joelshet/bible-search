// Offline after the first visit: everything the app needs is cached up front.
const VERSION = "0113ee743ef5";
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
  // Fetch past the HTTP cache: a stale copy saved under a new VERSION would be served from then on.
  const fresh = FILES.map((f) => new Request(f, { cache: "reload" }));
  e.waitUntil(caches.open(VERSION).then((c) => c.addAll(fresh)).then(() => self.skipWaiting()));
});

self.addEventListener("activate", (e) => {
  e.waitUntil(
    caches.keys()
      .then((keys) => Promise.all(keys.filter((k) => k !== VERSION).map((k) => caches.delete(k))))
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
