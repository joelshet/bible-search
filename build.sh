#!/bin/sh
# Rebuild everything in web/ from data/ and src/. Needs Rust (with the
# wasm32-unknown-unknown target) and wasm-pack. The site itself needs neither.
set -eu
cd "$(dirname "$0")"

cargo test --release --quiet --lib
cargo build --release --quiet --bins
mkdir -p target/index web/data
./target/release/build_index data target/index
gzip -9nc target/index/bible.idx > web/data/bible.idx.gz
gzip -9nc target/index/lexicon.idx > web/data/lexicon.idx.gz

wasm-pack build --release --target web --out-dir web/pkg --no-typescript --no-pack
rm -f web/pkg/.gitignore

# A new cache name whenever anything the app ships changes, so visitors get the update.
hash=$(cat web/index.html web/style.css web/app.js web/map.js web/reader.js web/worker.js web/pkg/* web/data/*.gz web/fonts/*.woff2 | shasum | cut -c1-12)
sed "s/^const VERSION = .*/const VERSION = \"$hash\";/" web/sw.js > web/sw.js.tmp && mv web/sw.js.tmp web/sw.js

ls -l web/data web/pkg
echo "built $hash. Check ranking with: ./target/release/eval target/index/bible.idx"
