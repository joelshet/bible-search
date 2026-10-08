//! The in-memory Bible and its binary file format. The builder fills a `Bible`
//! and calls `encode`; the browser calls `decode` on the same bytes.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// FNV-1a: far faster than the default SipHash for short word keys, and nothing here is adversarial.
#[derive(Default)]
pub struct Fnv(u64);

impl Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        let mut h = if self.0 == 0 { 0xcbf29ce484222325 } else { self.0 };
        for &b in bytes {
            h = (h ^ b as u64).wrapping_mul(0x100000001b3);
        }
        self.0 = h;
    }
}

pub type Map<K, V> = HashMap<K, V, BuildHasherDefault<Fnv>>;

pub const MAGIC: &[u8; 8] = b"KJVIDX04";

/// Strong's numbers packed into u16: H1..H8674 stay as is, G1..G5624 get +10000.
pub fn strongs_code(s: &str) -> Option<u16> {
    let (lang, n) = s.split_at_checked(1)?;
    let n: u16 = n.parse().ok().filter(|&n| n > 0)?;
    match lang {
        "H" | "h" => (n < 10000).then_some(n),
        "G" | "g" => (n < 10000).then_some(10000 + n),
        _ => None,
    }
}

pub fn strongs_name(code: u16) -> String {
    if code > 10000 { format!("G{}", code - 10000) } else { format!("H{code}") }
}

pub const ITALIC: u8 = 1;
pub const JESUS: u8 = 2;

#[derive(Default)]
pub struct Entry {
    pub lemma: String,
    pub translit: String,
    pub pron: String,
    pub derivation: String,
    pub definition: String,
    pub kjv_usage: String,
}

/// Postings: for each term, verse ids with term frequency.
#[derive(Default)]
pub struct Postings {
    pub offsets: Vec<u32>,
    pub verses: Vec<u16>,
    pub tfs: Vec<u8>,
}

impl Postings {
    pub fn get(&self, term: usize) -> (&[u16], &[u8]) {
        if term + 1 >= self.offsets.len() {
            return (&[], &[]);
        }
        let (a, b) = (self.offsets[term] as usize, self.offsets[term + 1] as usize);
        (&self.verses[a..b], &self.tfs[a..b])
    }
    pub fn df(&self, term: usize) -> usize {
        self.get(term).0.len()
    }
    pub fn from_lists(lists: Vec<Vec<(u16, u8)>>) -> Self {
        let mut p = Postings { offsets: vec![0], ..Default::default() };
        for list in lists {
            for (v, tf) in list {
                p.verses.push(v);
                p.tfs.push(tf);
            }
            p.offsets.push(p.verses.len() as u32);
        }
        p
    }
}

#[derive(Default)]
pub struct Bible {
    // verses, in canonical order
    pub book: Vec<u8>,
    pub chapter: Vec<u8>,
    pub verse: Vec<u8>,
    pub text: String,
    pub text_off: Vec<u32>,
    // one entry per word in each verse, in the order text::words finds them
    pub word_flags: Vec<u8>,
    pub word_strongs_off: Vec<u32>,
    pub word_strongs: Vec<u16>,
    /// Psalm superscriptions: (index of first verse of that chapter, title)
    pub titles: Vec<(u32, String)>,
    // vocabulary: every distinct normalized surface word, sorted
    pub surface: Vec<String>,
    pub surface_stem: Vec<u32>,
    pub stems: Vec<String>,
    /// Transliteration of each Strong's code, kept in the core file for match reasons.
    pub translit_of: Vec<String>,
    /// Accent-folded transliteration ("agape") to Strong's codes.
    pub translit: Vec<(String, Vec<u16>)>,
    /// Stem of a modern English word to KJV stems that can stand in for it, weight 0-100.
    pub synonyms: Vec<(String, Vec<(u32, u8)>)>,
    /// Full Strong's entries; a separate file, loaded after search is ready.
    pub lexicon: Vec<Entry>,

    // derived on load from the above
    pub word_off: Vec<u32>,
    pub word_stem: Vec<u32>,
    pub surface_count: Vec<u32>,
    pub stem_postings: Postings,
    pub strongs_postings: Postings,
    /// For each stem, the Strong's numbers behind it and what share of its uses each covers (0-255).
    pub stem_strongs: Vec<Vec<(u16, u8)>>,
    /// For each Strong's code, the English words the KJV used for it, most frequent first.
    pub renderings: Vec<Vec<(u32, u32)>>,
    pub surface_index: Map<String, u32>,
    pub stem_index: Map<String, u32>,
    pub translit_index: Map<String, usize>,
    pub synonym_index: Map<String, usize>,
    pub chapter_start: Vec<u32>,
}

pub const CODES: usize = 15700;

impl Bible {
    pub fn len(&self) -> usize {
        self.book.len()
    }
    pub fn verse_text(&self, v: usize) -> &str {
        &self.text[self.text_off[v] as usize..self.text_off[v + 1] as usize]
    }
    pub fn word_range(&self, v: usize) -> std::ops::Range<usize> {
        self.word_off[v] as usize..self.word_off[v + 1] as usize
    }
    pub fn strongs_of_word(&self, w: usize) -> &[u16] {
        &self.word_strongs[self.word_strongs_off[w] as usize..self.word_strongs_off[w + 1] as usize]
    }
    pub fn verse_len(&self, v: usize) -> usize {
        self.word_range(v).len()
    }
    pub fn entry(&self, code: u16) -> Option<&Entry> {
        self.lexicon.get(code as usize).filter(|e| !e.lemma.is_empty() || !e.definition.is_empty())
    }
    pub fn reference(&self, v: usize) -> String {
        format!("{} {}:{}", crate::books::BOOKS[self.book[v] as usize].name, self.chapter[v], self.verse[v])
    }
    pub fn find_verse(&self, book: usize, chapter: u32, verse: u32) -> Option<usize> {
        let c = self.chapter_start.iter().position(|&s| {
            self.book[s as usize] as usize == book && self.chapter[s as usize] as u32 == chapter
        })?;
        let v = self.chapter_start[c] as usize + verse.max(1) as usize - 1;
        (v < self.len() && self.chapter[v] as u32 == chapter && self.book[v] as usize == book).then_some(v)
    }
    pub fn chapter_of(&self, v: usize) -> usize {
        self.chapter_start.partition_point(|&s| s as usize <= v) - 1
    }
    pub fn chapter_range(&self, c: usize) -> std::ops::Range<usize> {
        let end = self.chapter_start.get(c + 1).map_or(self.len(), |&e| e as usize);
        self.chapter_start[c] as usize..end
    }

    /// Rebuild everything that can be computed from the text and its word tags.
    /// Shipping these would roughly double the download.
    pub fn derive(&mut self) {
        let n = self.len();
        self.surface_index = self.surface.iter().enumerate().map(|(i, s)| (s.clone(), i as u32)).collect();
        self.stem_index = self.stems.iter().enumerate().map(|(i, s)| (s.clone(), i as u32)).collect();
        self.translit_index = self.translit.iter().enumerate().map(|(i, (s, _))| (s.clone(), i)).collect();
        self.synonym_index = self.synonyms.iter().enumerate().map(|(i, (s, _))| (s.clone(), i)).collect();
        self.chapter_start = (0..n)
            .filter(|&v| v == 0 || self.chapter[v] != self.chapter[v - 1] || self.book[v] != self.book[v - 1])
            .map(|v| v as u32)
            .collect();

        let mut word_off = Vec::with_capacity(n + 1);
        let mut word_stem = Vec::with_capacity(self.word_flags.len());
        let mut surface_count = vec![0u32; self.surface.len()];
        let mut stem_lists: Vec<Vec<(u16, u8)>> = vec![vec![]; self.stems.len()];
        let mut code_lists: Vec<Vec<(u16, u8)>> = vec![vec![]; CODES];
        let mut stem_total = vec![0u32; self.stems.len()];
        let mut stem_code: Map<(u32, u16), u32> = Map::default();
        let mut code_surface: Map<(u16, u32), u32> = Map::default();
        let bump = |list: &mut Vec<(u16, u8)>, v: u16| match list.last_mut() {
            Some((last, tf)) if *last == v => *tf = tf.saturating_add(1),
            _ => list.push((v, 1)),
        };
        let mut buf = String::new();
        word_off.push(0);
        for v in 0..n {
            let t = &self.text[self.text_off[v] as usize..self.text_off[v + 1] as usize];
            for (a, b) in crate::text::words(t) {
                let w = word_stem.len();
                crate::text::normalize_into(&t[a..b], &mut buf);
                let s = self.surface_index[buf.as_str()];
                let st = self.surface_stem[s as usize];
                word_stem.push(st);
                surface_count[s as usize] += 1;
                stem_total[st as usize] += 1;
                bump(&mut stem_lists[st as usize], v as u16);
                let codes = &self.word_strongs[self.word_strongs_off[w] as usize..self.word_strongs_off[w + 1] as usize];
                for &c in codes {
                    bump(&mut code_lists[c as usize], v as u16);
                    *stem_code.entry((st, c)).or_default() += 1;
                    *code_surface.entry((c, s)).or_default() += 1;
                }
            }
            word_off.push(word_stem.len() as u32);
        }
        assert_eq!(word_stem.len(), self.word_flags.len(), "word tags out of step with text");
        self.word_off = word_off;
        self.word_stem = word_stem;
        self.surface_count = surface_count;
        self.stem_postings = Postings::from_lists(stem_lists);
        self.strongs_postings = Postings::from_lists(code_lists);

        self.stem_strongs = vec![vec![]; self.stems.len()];
        for (&(st, c), &k) in &stem_code {
            let share = k as f64 / stem_total[st as usize] as f64;
            if share >= 0.05 {
                self.stem_strongs[st as usize].push((c, (share * 255.0).round() as u8));
            }
        }
        for l in &mut self.stem_strongs {
            l.sort_by_key(|&(c, s)| (std::cmp::Reverse(s), c));
            l.truncate(6);
        }
        self.renderings = vec![vec![]; CODES];
        for (&(c, s), &k) in &code_surface {
            self.renderings[c as usize].push((s, k));
        }
        for l in &mut self.renderings {
            l.sort_by_key(|&(s, k)| (std::cmp::Reverse(k), s));
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer(MAGIC.to_vec());
        w.bytes(&self.book);
        w.bytes(&self.chapter);
        w.bytes(&self.verse);
        w.str(&self.text);
        w.deltas(&self.text_off);
        w.bytes(&self.word_flags);
        // most words carry zero or one code: store count, then codes
        let mut counts = Vec::with_capacity(self.word_flags.len());
        for i in 0..self.word_flags.len() {
            counts.push((self.word_strongs_off[i + 1] - self.word_strongs_off[i]) as u8);
        }
        w.bytes(&counts);
        w.u16s(&self.word_strongs);
        w.n(self.titles.len());
        for (v, t) in &self.titles {
            w.n(*v as usize);
            w.str(t);
        }
        w.strs(&self.surface);
        w.u32s(&self.surface_stem);
        w.strs(&self.stems);
        w.strs(&self.translit_of);
        w.n(self.translit.len());
        for (s, codes) in &self.translit {
            w.str(s);
            w.u16s(codes);
        }
        w.n(self.synonyms.len());
        for (s, list) in &self.synonyms {
            w.str(s);
            w.n(list.len());
            for &(t, wt) in list {
                w.n(t as usize);
                w.n(wt as usize);
            }
        }
        w.0
    }

    pub fn decode(data: &[u8]) -> Result<Bible, String> {
        if data.get(..8) != Some(&MAGIC[..]) {
            return Err("not a KJV index (or an old version)".into());
        }
        let mut r = Reader { d: data, p: 8 };
        let mut b = Bible {
            book: r.bytes()?,
            chapter: r.bytes()?,
            verse: r.bytes()?,
            text: r.str()?,
            text_off: r.deltas()?,
            word_flags: r.bytes()?,
            ..Default::default()
        };
        let counts = r.bytes()?;
        b.word_strongs_off = Vec::with_capacity(counts.len() + 1);
        let mut at = 0u32;
        b.word_strongs_off.push(0);
        for c in counts {
            at += c as u32;
            b.word_strongs_off.push(at);
        }
        b.word_strongs = r.u16s()?;
        for _ in 0..r.n()? {
            let v = r.n()? as u32;
            b.titles.push((v, r.str()?));
        }
        b.surface = r.strs()?;
        b.surface_stem = r.u32s()?;
        b.stems = r.strs()?;
        b.translit_of = r.strs()?;
        for _ in 0..r.n()? {
            let s = r.str()?;
            b.translit.push((s, r.u16s()?));
        }
        for _ in 0..r.n()? {
            let s = r.str()?;
            let mut list = Vec::new();
            for _ in 0..r.n()? {
                list.push((r.n()? as u32, r.n()? as u8));
            }
            b.synonyms.push((s, list));
        }
        b.derive();
        Ok(b)
    }

    pub fn encode_lexicon(&self) -> Vec<u8> {
        let mut w = Writer(LEXICON_MAGIC.to_vec());
        w.n(self.lexicon.len());
        for e in &self.lexicon {
            for s in [&e.lemma, &e.translit, &e.pron, &e.derivation, &e.definition, &e.kjv_usage] {
                w.str(s);
            }
        }
        w.0
    }

    pub fn load_lexicon(&mut self, data: &[u8]) -> Result<(), String> {
        if data.get(..8) != Some(&LEXICON_MAGIC[..]) {
            return Err("not a Strong's lexicon file".into());
        }
        let mut r = Reader { d: data, p: 8 };
        self.lexicon = (0..r.n()?)
            .map(|_| {
                Ok(Entry {
                    lemma: r.str()?,
                    translit: r.str()?,
                    pron: r.str()?,
                    derivation: r.str()?,
                    definition: r.str()?,
                    kjv_usage: r.str()?,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(())
    }
}

pub const LEXICON_MAGIC: &[u8; 8] = b"KJVLEX01";

struct Writer(Vec<u8>);

impl Writer {
    fn n(&mut self, mut v: usize) {
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                self.0.push(b);
                return;
            }
            self.0.push(b | 0x80);
        }
    }
    fn bytes(&mut self, b: &[u8]) {
        self.n(b.len());
        self.0.extend_from_slice(b);
    }
    fn str(&mut self, s: &str) {
        self.bytes(s.as_bytes());
    }
    fn strs(&mut self, list: &[String]) {
        self.n(list.len());
        for s in list {
            self.str(s);
        }
    }
    fn u16s(&mut self, list: &[u16]) {
        self.n(list.len());
        for &v in list {
            self.n(v as usize);
        }
    }
    fn u32s(&mut self, list: &[u32]) {
        self.n(list.len());
        for &v in list {
            self.n(v as usize);
        }
    }
    fn deltas(&mut self, list: &[u32]) {
        self.n(list.len());
        let mut last = 0;
        for &v in list {
            self.n((v - last) as usize);
            last = v;
        }
    }
}

struct Reader<'a> {
    d: &'a [u8],
    p: usize,
}

impl Reader<'_> {
    fn n(&mut self) -> Result<usize, String> {
        let mut v = 0usize;
        let mut shift = 0;
        loop {
            let b = *self.d.get(self.p).ok_or("index file is truncated")?;
            self.p += 1;
            v |= ((b & 0x7f) as usize) << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
            shift += 7;
        }
    }
    fn raw(&mut self, len: usize) -> Result<Vec<u8>, String> {
        let s = self.d.get(self.p..self.p + len).ok_or("index file is truncated")?;
        self.p += len;
        Ok(s.to_vec())
    }
    fn bytes(&mut self) -> Result<Vec<u8>, String> {
        let len = self.n()?;
        self.raw(len)
    }
    fn str(&mut self) -> Result<String, String> {
        String::from_utf8(self.bytes()?).map_err(|e| e.to_string())
    }
    fn strs(&mut self) -> Result<Vec<String>, String> {
        (0..self.n()?).map(|_| self.str()).collect()
    }
    fn u16s(&mut self) -> Result<Vec<u16>, String> {
        (0..self.n()?).map(|_| self.n().map(|v| v as u16)).collect()
    }
    fn u32s(&mut self) -> Result<Vec<u32>, String> {
        (0..self.n()?).map(|_| self.n().map(|v| v as u32)).collect()
    }
    fn deltas(&mut self) -> Result<Vec<u32>, String> {
        let len = self.n()?;
        let mut out = Vec::with_capacity(len);
        let mut last = 0u32;
        for _ in 0..len {
            last += self.n()? as u32;
            out.push(last);
        }
        Ok(out)
    }
}
