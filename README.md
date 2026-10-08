# Bible Search

Search the King James Bible the way you remember it, then read it. "eagles not getting tired" finds Isaiah 40:31; "plans to prosper you and not to harm you" finds Jeremiah 29:11, where the KJV says "thoughts of peace, and not of evil". Every search runs in the browser in Rust compiled to WebAssembly. The whole Bible is drawn as one zoomable map, a mosaic of 31,102 verse tiles that zooms down to readable text, and a reader with a contents page shows chapter after chapter with your search still underlined.

## Run it

```sh
python3 -m http.server -d web 8740
```

Then open http://localhost:8740. The `web/` folder is the whole site: static files with no server code. It works under any path (for example `/bible-search/`), and after the first visit it works offline.

## Rebuild it

You only need this after changing `src/` or `data/`. It requires Rust with the `wasm32-unknown-unknown` target, plus `wasm-pack`.

```sh
./build.sh
./target/release/eval target/index/bible.idx                      # ranking report
./target/release/eval target/index/bible.idx "eagles not getting tired"  # one query, with reasons
```

`tools/prepare.py` downloads the three sources and flattens them into `data/*.tsv`. Its outputs are committed, so `build.sh` never touches the network.

## How it works

- `src/text.rs`: the tokenizer and a stemmer that knows 1611 English (runneth, saith, spake). The index and the queries share it.
- `src/search.rs`: expands each word into its forms, completions while you type, the Strong's numbers behind it, WordNet and hand-written synonyms (`data/modern.tsv`), and spelling fixes. Ranking is BM25 with a bonus for verses that contain every word and keep the words in order. Function words only boost verses that already matched.
- `src/index.rs`: the binary format. The download carries the text, per-word Strong's tags, the vocabulary, and synonyms. Postings and rendering counts are rebuilt on load, which halves the download.
- `src/layout.rs`: the map. Each verse's area is proportional to its length, so text fits at one size everywhere. The Old Testament runs across the top, the New Testament along the bottom, and each section, book, and chapter is split in reading order.
- `web/worker.js` runs the engine off the main thread and drops keystrokes that have already been superseded. `web/map.js` draws the map on a canvas, `web/reader.js` handles the contents page and continuous reading, and `web/app.js` handles everything else.

## Numbers (October 2026, M-series Mac)

- Download: 2.3 MB for search (`bible.idx.gz`), 0.1 MB for the wasm, plus a 0.8 MB lexicon that loads after search is ready.
- Engine load: 87 ms in Chrome after download.
- Search: 0.2 to 1.4 ms per query in Chrome once warm. The first query or two run slower while the browser optimizes the wasm.
- Half-remembered queries (`src/bin/eval.rs`): 27 of 29 at rank 1 on the tuned set, and 20 of 25 at rank 1 (22 of 25 in the top 3) on a held-out set written after tuning and never tuned against. Some entries in `data/modern.tsv` came from looking at tuned-set misses, so the held-out number is the honest one.

## Sources and licenses

- King James Version, 1769 text, with Strong's numbers: public domain, from eBible.org and the CrossWire Bible Society.
- Strong's Hebrew and Greek dictionaries: Open Scriptures, CC BY-SA. These are in `data/strongs.tsv` and `web/data/lexicon.idx.gz`.
- WordNet 3.0, Copyright 2006 by Princeton University, used under the WordNet license. Its license text is in `data/raw/WNdb-3.0.tar.gz` after running `prepare.py`, and it's reproduced here:

  > Permission to use, copy, modify and distribute this software and database and its documentation for any purpose and without fee or royalty is hereby granted, provided that you agree to comply with the following copyright notice and statements, including the disclaimer, and that the same appear on ALL copies of the software, database and documentation, including modifications that you make for internal use or for distribution. WordNet 3.0 Copyright 2006 by Princeton University. All rights reserved. THIS SOFTWARE AND DATABASE IS PROVIDED "AS IS" AND PRINCETON UNIVERSITY MAKES NO REPRESENTATIONS OR WARRANTIES, EXPRESS OR IMPLIED. BY WAY OF EXAMPLE, BUT NOT LIMITATION, PRINCETON UNIVERSITY MAKES NO REPRESENTATIONS OR WARRANTIES OF MERCHANTABILITY OR FITNESS FOR ANY PARTICULAR PURPOSE OR THAT THE USE OF THE LICENSED SOFTWARE, DATABASE OR DOCUMENTATION WILL NOT INFRINGE ANY THIRD PARTY PATENTS, COPYRIGHTS, TRADEMARKS OR OTHER RIGHTS. The name of Princeton University or Princeton may not be used in advertising or publicity pertaining to distribution of the software and/or database. Title to copyright in this software, database and any associated documentation shall at all times remain with Princeton University and LICENSEE agrees to preserve same.

- EB Garamond font: The EB Garamond Project Authors, SIL Open Font License (`web/fonts/OFL-garamond.txt`).
- Atkinson Hyperlegible font: Braille Institute, SIL Open Font License (`web/fonts/OFL-atkinson.txt`).
- `qrcodegen` (Project Nayuki, MIT) and `wasm-bindgen` (MIT/Apache-2.0) are the only Rust dependencies.

## Outside this folder

Nothing at runtime. Visitors' browsers keep a service-worker cache and their reading settings in local storage for this site. For local previews, `~/repos/.claude/launch.json` has a `bible-app` entry; delete that entry to remove it.
