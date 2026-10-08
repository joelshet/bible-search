//! Reads data/*.tsv and writes the binary index the browser loads.
//!
//!   cargo run --release --bin build_index -- data web/data

use bible::index::{strongs_code, Bible, Entry, CODES, ITALIC, JESUS};
use bible::{books, text};
use std::collections::HashMap;
use std::{env, fs, process};

fn main() {
    let args: Vec<String> = env::args().collect();
    let (dir, out) = match args.as_slice() {
        [_, dir, out] => (dir.as_str(), out.as_str()),
        _ => {
            eprintln!("usage: build_index DATA_DIR OUT_DIR");
            process::exit(2);
        }
    };
    let read = |name: &str| {
        fs::read_to_string(format!("{dir}/{name}")).unwrap_or_else(|e| {
            eprintln!("{dir}/{name}: {e}");
            process::exit(1);
        })
    };
    let bible = build(&read("kjv.tsv"), &read("strongs.tsv"), &read("wordnet.tsv"), &read("modern.tsv"));
    let bytes = bible.encode();
    let write = |name: &str, data: &[u8]| {
        fs::write(format!("{out}/{name}"), data).unwrap_or_else(|e| {
            eprintln!("{out}/{name}: {e}");
            process::exit(1);
        })
    };
    write("bible.idx", &bytes);
    write("lexicon.idx", &bible.encode_lexicon());
    eprintln!(
        "bible.idx: {} verses, {} words, {} surface forms, {} stems, {} synonym keys, {:.2} MB",
        bible.len(),
        bible.word_flags.len(),
        bible.surface.len(),
        bible.stems.len(),
        bible.synonyms.len(),
        bytes.len() as f64 / 1e6
    );
}

/// A verse with its markup stripped: plain text plus attribute ranges over it.
struct Parsed {
    text: String,
    strongs: Vec<(usize, usize, Vec<u16>)>,
    italic: Vec<(usize, usize)>,
    jesus: Vec<(usize, usize)>,
}

fn parse_markup(src: &str) -> Parsed {
    let mut p = Parsed { text: String::new(), strongs: vec![], italic: vec![], jesus: vec![] };
    let (mut span, mut it, mut wj) = (None, Vec::new(), None);
    let mut chars = src.chars();
    while let Some(c) = chars.next() {
        match c {
            '{' => span = Some(p.text.len()),
            '|' => {
                let codes: String = chars.by_ref().take_while(|&c| c != '}').collect();
                let codes = codes.split(',').filter_map(strongs_code).collect();
                p.strongs.push((span.take().expect("| outside {}"), p.text.len(), codes));
            }
            '[' => it.push(p.text.len()),
            ']' => p.italic.push((it.pop().expect("unmatched ]"), p.text.len())),
            '<' => wj = Some(p.text.len()),
            '>' => p.jesus.push((wj.take().expect("unmatched >"), p.text.len())),
            _ => p.text.push(c),
        }
    }
    if let Some(start) = wj {
        p.jesus.push((start, p.text.len()));
    }
    p
}

fn inside(ranges: &[(usize, usize)], at: usize) -> bool {
    ranges.iter().any(|&(a, b)| a <= at && at < b)
}

fn build(kjv: &str, strongs_tsv: &str, wordnet: &str, modern: &str) -> Bible {
    let mut b = Bible { text_off: vec![0], word_strongs_off: vec![0], ..Default::default() };
    let mut surface_index: HashMap<String, u32> = HashMap::new();
    let mut pending_title = None;

    for line in kjv.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let book = books::by_code(f[0]).expect("unknown book code") as u8;
        let (chapter, verse): (u8, u8) = (f[1].parse().unwrap(), f[2].parse().unwrap());
        let p = parse_markup(f[3]);
        if verse == 0 {
            pending_title = Some(p.text);
            continue;
        }
        if let Some(t) = pending_title.take() {
            b.titles.push((b.book.len() as u32, t));
        }
        b.book.push(book);
        b.chapter.push(chapter);
        b.verse.push(verse);

        let words = text::words(&p.text);
        for &(start, end) in &words {
            let next = surface_index.len() as u32;
            surface_index.entry(text::normalize(&p.text[start..end])).or_insert(next);
            let mut flags = 0;
            if inside(&p.italic, start) {
                flags |= ITALIC;
            }
            if inside(&p.jesus, start) {
                flags |= JESUS;
            }
            b.word_flags.push(flags);
            // A tag over "seek ye" belongs to "seek"; "ye" only gets it if every word is a function word.
            if let Some((a, z, codes)) = p.strongs.iter().find(|(a, z, _)| *a <= start && start < *z) {
                let span_words: Vec<_> = words.iter().filter(|(s, _)| a <= s && s < z).collect();
                let content = |&&(s, e): &&(usize, usize)| !text::is_stop(&text::stem(&p.text[s..e]));
                let any_content = span_words.iter().any(content);
                if !any_content || content(&&(start, end)) {
                    b.word_strongs.extend(codes);
                }
            }
            b.word_strongs_off.push(b.word_strongs.len() as u32);
        }
        b.text.push_str(&p.text);
        b.text_off.push(b.text.len() as u32);
    }

    // Vocabulary, sorted so prefix lookups can binary search.
    let mut by_name: Vec<String> = surface_index.into_keys().collect();
    by_name.sort();
    b.surface = by_name;
    let mut stem_index: HashMap<String, u32> = HashMap::new();
    for s in &b.surface {
        let st = text::stem(s);
        let next = stem_index.len() as u32;
        let id = *stem_index.entry(st.clone()).or_insert_with(|| {
            b.stems.push(st);
            next
        });
        b.surface_stem.push(id);
    }
    b.derive();

    // Strong's lexicon, and transliterations that aren't already English words in the KJV.
    b.lexicon = (0..CODES).map(|_| Entry::default()).collect();
    b.translit_of = vec![String::new(); CODES];
    let mut translit: HashMap<String, Vec<u16>> = HashMap::new();
    for line in strongs_tsv.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let Some(code) = strongs_code(f[0]) else { continue };
        b.lexicon[code as usize] = Entry {
            lemma: f[1].into(),
            translit: f[2].into(),
            pron: f[3].into(),
            derivation: f[4].into(),
            definition: f[5].into(),
            kjv_usage: f[6].into(),
        };
        if b.strongs_postings.df(code as usize) == 0 {
            continue;
        }
        b.translit_of[code as usize] = f[2].into();
        let t = text::normalize(f[2]);
        if t.len() >= 3 && b.surface.binary_search(&t).is_err() {
            translit.entry(t).or_default().push(code);
        }
    }
    b.translit = translit.into_iter().collect();
    b.translit.sort();

    // Modern English to KJV vocabulary: WordNet first, then the hand-written list on top.
    let mut syn: HashMap<String, HashMap<u32, u8>> = HashMap::new();
    let mut add = |key: &str, word: &str, weight: u8| {
        let (k, s) = (text::stem(key), text::stem(word));
        if let Some(&id) = stem_index.get(&s) {
            if k != s && !text::is_stop(&s) {
                let e = syn.entry(k).or_default().entry(id).or_default();
                *e = (*e).max(weight);
            }
        }
    };
    for line in wordnet.lines() {
        let (key, rest) = line.split_once('\t').unwrap();
        for item in rest.split(' ') {
            let (word, w) = item.split_once(':').unwrap();
            let w: u8 = w.parse().unwrap();
            if w >= 25 {
                add(key, word, w);
            }
        }
    }
    for line in modern.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let (key, rest) = line.split_once('\t').expect("modern.tsv: word<TAB>kjv words");
        for word in rest.split_whitespace() {
            add(key, word, 100);
        }
    }
    let mut synonyms: Vec<(String, Vec<(u32, u8)>)> = syn
        .into_iter()
        .map(|(k, m)| {
            let mut l: Vec<(u32, u8)> = m.into_iter().collect();
            l.sort_by_key(|&(id, w)| (std::cmp::Reverse(w), id));
            l.truncate(12);
            (k, l)
        })
        .collect();
    synonyms.sort();
    b.synonyms = synonyms;
    b.derive();
    b
}
