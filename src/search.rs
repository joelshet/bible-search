//! Query parsing, expansion (forms, typos, synonyms, Strong's), and ranking.

use crate::books::{self, Reference};
use crate::index::{strongs_code, strongs_name, Bible};
use crate::text;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Why {
    Stop,
    Exact,
    Form,
    Prefix,
    Typo,
    Synonym,
    Strongs,
}

impl Why {
    pub fn code(self) -> &'static str {
        match self {
            Why::Stop => "stop",
            Why::Exact => "exact",
            Why::Form => "form",
            Why::Prefix => "prefix",
            Why::Typo => "typo",
            Why::Synonym => "synonym",
            Why::Strongs => "strongs",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Target {
    Stem(u32),
    Strongs(u16),
}

#[derive(Clone, Debug)]
pub struct Expansion {
    pub target: Target,
    pub weight: f32,
    pub why: Why,
}

#[derive(Clone, Debug)]
pub struct Term {
    pub typed: String,
    pub stop: bool,
    pub required: bool,
    pub expansions: Vec<Expansion>,
}

#[derive(Default)]
pub struct Query {
    pub terms: Vec<Term>,
    pub excluded: Vec<Target>,
    pub phrases: Vec<String>,
    pub scope: Option<Vec<bool>>,
    pub reference: Option<Reference>,
    pub book_hint: Option<usize>,
    /// "nebuchadnezer → nebuchadnezzar" and similar, shown above results
    pub notes: Vec<String>,
}

pub struct Results {
    pub query: Query,
    /// Each result's first verse.
    pub ids: Vec<u32>,
    /// How many verses each result runs for: 1, or more for a passage.
    pub spans: Vec<u8>,
    pub scores: Vec<f32>,
    pub full: Vec<bool>,
    pub full_count: usize,
    /// Verses containing every content word itself (or its forms), before synonyms and Strong's.
    pub literal_count: usize,
    pub books: usize,
    pub hits: Vec<u8>,
    targets: HashMap<Target, (usize, f32, Why)>,
}

const K1: f32 = 1.2;
const B: f32 = 0.5;

/// How many of the best verses get the closer look: word order, and runs.
const CLOSE: usize = 300;
/// A passage remembered across several verses is one result: a run of neighbouring verses
/// that between them hold the query. Runs grow from the best single verses.
const MAX_RUN: usize = 4;
/// What each verse past the first costs a run, so one verse that holds the query still wins.
const RUN_COST: f32 = 0.8;

struct Cand {
    first: u32,
    span: u8,
    score: f32,
    full: bool,
}

/// Letters and spaces only, for phrase containment checks.
fn flatten(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push(' ');
    for (a, b) in text::words(s) {
        out.push_str(&text::normalize(&s[a..b]));
        out.push(' ');
    }
    out
}

impl Bible {
    pub fn stem_id(&self, word: &str) -> Option<u32> {
        match self.surface_index.get(&text::normalize(word)) {
            Some(&s) => Some(self.surface_stem[s as usize]),
            None => self.stem_index.get(&text::stem(word)).copied(),
        }
    }

    fn idf(&self, t: Target) -> f32 {
        let df = match t {
            Target::Stem(s) => self.stem_postings.df(s as usize),
            Target::Strongs(c) => self.strongs_postings.df(c as usize),
        } as f32;
        let n = self.len() as f32;
        (1.0 + (n - df + 0.5) / (df + 0.5)).ln()
    }

    fn postings(&self, t: Target) -> (&[u16], &[u8]) {
        match t {
            Target::Stem(s) => self.stem_postings.get(s as usize),
            Target::Strongs(c) => self.strongs_postings.get(c as usize),
        }
    }

    /// Every way a typed word can match: its own forms, completions while typing,
    /// Strong's numbers behind it, synonyms, and typo corrections when nothing else fits.
    pub fn expand(&self, word: &str, live: bool, notes: &mut Vec<String>) -> Vec<Expansion> {
        let mut out: Vec<Expansion> = Vec::new();
        let push = |out: &mut Vec<Expansion>, target, weight: f32, why| {
            if let Some(e) = out.iter_mut().find(|e: &&mut Expansion| e.target == target) {
                if weight > e.weight {
                    e.weight = weight;
                    e.why = why;
                }
            } else {
                out.push(Expansion { target, weight, why });
            }
        };
        if let Some(code) = strongs_code(word) {
            if self.strongs_postings.df(code as usize) > 0 {
                push(&mut out, Target::Strongs(code), 1.0, Why::Strongs);
            }
            return out;
        }
        let norm = text::normalize(word);
        if norm.is_empty() {
            return out;
        }
        let stem = text::stem(&norm);
        let own = self.stem_index.get(&stem).copied();
        // Words the KJV barely uses ("tired" appears once) lean on synonyms, not on their few Strong's tags.
        let modern = own.is_none_or(|s| self.stem_postings.df(s as usize) < 5);
        if let Some(s) = own {
            push(&mut out, Target::Stem(s), 1.0, Why::Exact);
            if !text::is_stop(&stem) && !modern {
                for &(code, share) in &self.stem_strongs[s as usize] {
                    let share = share as f32 / 255.0;
                    if share >= 0.12 {
                        push(&mut out, Target::Strongs(code), 0.75 * share.sqrt(), Why::Strongs);
                    }
                }
            }
        }
        if let Some(&i) = self.translit_index.get(&norm) {
            for &code in &self.translit[i].1 {
                push(&mut out, Target::Strongs(code), 1.0, Why::Strongs);
            }
        }
        if !text::is_stop(&stem) {
            if let Some(&i) = self.synonym_index.get(&stem) {
                let scale = if modern { 0.8 } else { 0.5 };
                for &(t, w) in &self.synonyms[i].1 {
                    push(&mut out, Target::Stem(t), scale * w as f32 / 100.0, Why::Synonym);
                }
            }
        }
        if live && norm.len() >= 2 {
            let start = self.surface.partition_point(|s| s.as_str() < norm.as_str());
            let mut completions: Vec<usize> = (start..self.surface.len())
                .take_while(|&i| self.surface[i].starts_with(&norm))
                .filter(|&i| Some(self.surface_stem[i]) != own)
                .collect();
            completions.sort_by_key(|&i| std::cmp::Reverse(self.surface_count[i]));
            for &i in completions.iter().take(30) {
                push(&mut out, Target::Stem(self.surface_stem[i]), 0.85, Why::Prefix);
            }
        }
        if out.is_empty() && norm.len() >= 4 {
            let max = if norm.len() >= 8 { 2 } else { 1 };
            let mut fixes: Vec<(usize, usize)> = self
                .surface
                .iter()
                .enumerate()
                .filter_map(|(i, s)| text::edit_distance(norm.as_bytes(), s.as_bytes(), max).map(|d| (d, i)))
                .collect();
            fixes.sort_by_key(|&(d, i)| (d, std::cmp::Reverse(self.surface_count[i])));
            fixes.truncate(4);
            if let Some(&(_, best)) = fixes.first() {
                notes.push(format!("{norm} → {}", self.surface[best]));
            }
            for (d, i) in fixes {
                push(&mut out, Target::Stem(self.surface_stem[i]), if d == 1 { 0.8 } else { 0.6 }, Why::Typo);
            }
        }
        out
    }

    pub fn parse(&self, q: &str, live: bool) -> Query {
        let mut query = Query::default();
        let mut rest = String::new();
        let mut chars = q.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '"' || c == '“' || c == '”' {
                let phrase: String = chars.by_ref().take_while(|&c| c != '"' && c != '”').collect();
                if !phrase.trim().is_empty() {
                    query.phrases.push(phrase);
                }
            } else {
                rest.push(c);
            }
        }
        let mut words: Vec<String> = Vec::new();
        for token in rest.split_whitespace() {
            if let Some(scope) = token.strip_prefix("in:") {
                let groups = books::group(&scope.to_lowercase()).unwrap_or_default();
                let s = query.scope.get_or_insert_with(|| vec![false; books::BOOKS.len()]);
                for g in groups {
                    s[g] = true;
                }
            } else if let Some(neg) = token.strip_prefix('-').filter(|t| !t.is_empty()) {
                if let Some(code) = strongs_code(neg) {
                    query.excluded.push(Target::Strongs(code));
                } else if let Some(s) = self.stem_id(neg) {
                    query.excluded.push(Target::Stem(s));
                }
            } else {
                words.push(token.to_string());
            }
        }
        let plain = words.join(" ");
        if query.phrases.is_empty() {
            query.reference = books::parse_reference(&plain);
            if query.reference.is_some() {
                return query;
            }
            if !plain.chars().any(|c| c.is_ascii_digit()) {
                query.book_hint = books::find(&plain).filter(|_| plain.len() >= 3);
            }
        }
        let ends_open = live && !q.ends_with(char::is_whitespace) && !q.ends_with('"');
        let mut items: Vec<(String, bool, bool)> = Vec::new(); // word, required, live
        for p in &query.phrases {
            for (a, b) in text::words(p) {
                items.push((p[a..b].to_string(), true, false));
            }
        }
        let n_words = words.len();
        for (i, w) in words.iter().enumerate() {
            let explicit_prefix = w.ends_with('*');
            let w = w.trim_end_matches('*');
            if strongs_code(w).is_some() {
                items.push((w.to_string(), false, false));
                continue;
            }
            let parts = text::words(w);
            let last = parts.len().saturating_sub(1);
            for (j, (a, b)) in parts.into_iter().enumerate() {
                let open = j == last && (explicit_prefix || (ends_open && i + 1 == n_words));
                items.push((w[a..b].to_string(), false, open));
            }
        }
        let all_stop = items.iter().all(|(w, _, _)| text::is_stop(&text::stem(w)));
        for (word, required, open) in items {
            let stop = !required && !all_stop && text::is_stop(&text::stem(&word)) && strongs_code(&word).is_none();
            let expansions = if stop || required {
                self.stem_id(&word)
                    .map(|s| vec![Expansion { target: Target::Stem(s), weight: 1.0, why: Why::Exact }])
                    .unwrap_or_default()
            } else {
                self.expand(&word, open, &mut query.notes)
            };
            query.terms.push(Term { typed: text::normalize(&word), stop, required, expansions });
        }
        query
    }

    pub fn search(&self, q: &str, live: bool, canonical: bool) -> Results {
        let query = self.parse(q, live);
        let n = self.len();
        if let Some(r) = query.reference {
            return self.reference_results(query, r);
        }

        let avg_len = self.word_flags.len() as f32 / n as f32;
        let mut total = vec![0f32; n];
        let mut stop_total = vec![0f32; n];
        let mut mask = vec![0u32; n];
        let mut literal = vec![0u32; n];
        let mut cur = vec![0f32; n];
        let mut touched: Vec<usize> = Vec::new();
        let mut content_mask = 0u32;
        let mut required_mask = 0u32;
        let mut targets: HashMap<Target, (usize, f32, Why)> = HashMap::new();
        // Each term's score in each verse, kept only when runs of verses will be scored.
        // A quoted phrase asks for verses that hold it, so it rules runs out.
        let n_terms = query.terms.len().min(32);
        let runs = query.phrases.is_empty() && query.terms.iter().take(32).filter(|t| !t.stop).count() >= 2;
        let mut per = vec![0f32; if runs { n_terms * n } else { 0 }];

        for (i, term) in query.terms.iter().enumerate().take(32) {
            let bit = 1u32 << i;
            if !term.stop {
                content_mask |= bit;
            }
            if term.required {
                required_mask |= bit;
            }
            for e in &term.expansions {
                let idf = self.idf(e.target);
                let (verses, tfs) = self.postings(e.target);
                let own = e.why == Why::Exact;
                for (&v, &tf) in verses.iter().zip(tfs) {
                    let v = v as usize;
                    if own {
                        literal[v] |= bit;
                    }
                    let tf = tf as f32;
                    let norm = 1.0 - B + B * self.verse_len(v) as f32 / avg_len;
                    let s = e.weight * idf * tf * (K1 + 1.0) / (tf + K1 * norm);
                    if s > cur[v] {
                        if cur[v] == 0.0 {
                            touched.push(v);
                        }
                        cur[v] = s;
                    }
                }
                let entry = targets.entry(e.target).or_insert((i, 0.0, e.why));
                if e.weight > entry.1 {
                    *entry = (i, e.weight, e.why);
                }
            }
            for &v in &touched {
                if runs {
                    per[i * n + v] = cur[v];
                }
                if term.stop {
                    stop_total[v] += cur[v];
                } else {
                    total[v] += cur[v];
                    mask[v] |= bit;
                }
                cur[v] = 0.0;
            }
            touched.clear();
        }

        let excluded: Vec<bool> = {
            let mut ex = vec![false; n];
            for &t in &query.excluded {
                for &v in self.postings(t).0 {
                    ex[v as usize] = true;
                }
            }
            ex
        };
        let phrases: Vec<String> = query.phrases.iter().map(|p| flatten(p)).collect();
        let n_content = content_mask.count_ones().max(1) as f32;
        let mut cands: Vec<Cand> = Vec::new();
        let mut literal_count = 0;
        for v in 0..n {
            if mask[v] == 0 || excluded[v] || mask[v] & required_mask != required_mask {
                continue;
            }
            if let Some(scope) = &query.scope {
                if !scope[self.book[v] as usize] {
                    continue;
                }
            }
            if !phrases.is_empty() {
                let flat = flatten(self.verse_text(v));
                if !phrases.iter().all(|p| flat.contains(p.as_str())) {
                    continue;
                }
            }
            let m = (mask[v] & content_mask).count_ones() as f32;
            let score = total[v] * (m / n_content).powf(1.5) + 0.3 * stop_total[v];
            cands.push(Cand { first: v as u32, span: 1, score, full: m == n_content });
            if literal[v] & content_mask == content_mask {
                literal_count += 1;
            }
        }

        // Reward verses where the query's words sit next to each other in the same order.
        let stems: Vec<Option<u32>> = query.terms.iter().map(|t| self.stem_id(&t.typed)).collect();
        let pairs = stems.windows(2).filter(|p| p[0].is_some() && p[1].is_some()).count().max(1);
        let in_order = |first: usize, span: usize| {
            let words = &self.word_stem[self.word_off[first] as usize..self.word_off[first + span] as usize];
            let found = stems
                .windows(2)
                .filter(|p| p[0].is_some() && p[1].is_some() && words.windows(2).any(|w| Some(w[0]) == p[0] && Some(w[1]) == p[1]))
                .count();
            1.0 + 0.6 * found as f32 / pairs as f32
        };
        let by_score = |a: &Cand, b: &Cand| b.score.total_cmp(&a.score).then(a.first.cmp(&b.first));
        if stems.len() >= 2 {
            cands.sort_by(by_score);
            for c in cands.iter_mut().take(CLOSE) {
                c.score *= in_order(c.first as usize, 1);
            }
        }

        if runs {
            let mut seen: std::collections::HashSet<(usize, usize)> = Default::default();
            for seed in 0..cands.len().min(CLOSE) {
                let v = cands[seed].first as usize;
                let chapter = self.chapter_range(self.chapter_of(v));
                for span in 2..=MAX_RUN {
                    for s in (v + 1).saturating_sub(span).max(chapter.start)..=v {
                        let e = s + span - 1;
                        if e >= chapter.end || !seen.insert((s, span)) {
                            continue;
                        }
                        // Both end verses must hold two of the query's words, one of them a word the
                        // rest of the run lacks. A neighbour with a single stray word doesn't make a passage.
                        let inner = (s + 1..e).fold(0, |m, v| m | mask[v]);
                        let anchors = |end: usize, other: usize| {
                            mask[end].count_ones() >= 2 && mask[end] & !(inner | mask[other]) != 0
                        };
                        if !anchors(s, e) || !anchors(e, s) || (s..=e).any(|v| excluded[v]) {
                            continue;
                        }
                        let (mut content, mut stop, mut m) = (0f32, 0f32, 0f32);
                        for (i, term) in query.terms.iter().enumerate().take(32) {
                            let best = (s..=e).map(|v| per[i * n + v]).fold(0f32, f32::max);
                            if term.stop {
                                stop += best;
                            } else if best > 0.0 {
                                content += best;
                                m += 1.0;
                            }
                        }
                        let score = (content * (m / n_content).powf(1.5) + 0.3 * stop) * RUN_COST.powi(span as i32 - 1);
                        cands.push(Cand { first: s as u32, span: span as u8, score: score * in_order(s, span), full: m == n_content });
                    }
                }
            }
            // Best first, and a verse appears once: alone, or in the best run that holds it.
            cands.sort_by(by_score);
            let mut taken = vec![false; n];
            cands.retain(|c| {
                let range = c.first as usize..c.first as usize + c.span as usize;
                let free = !taken[range.clone()].iter().any(|&t| t);
                if free {
                    taken[range].fill(true);
                }
                free
            });
        }

        if canonical {
            cands.sort_by_key(|c| (!c.full, c.first));
        } else {
            cands.sort_by(by_score);
        }
        let top = cands.iter().map(|c| c.score).fold(0f32, f32::max).max(1e-6);
        let mut hits = vec![0u8; n];
        let mut book_seen = [false; 66];
        for c in &cands {
            let first = c.first as usize;
            hits[first..first + c.span as usize].fill(1 + (254.0 * (c.score / top).powf(0.7)) as u8);
            book_seen[self.book[first] as usize] = true;
        }
        Results {
            literal_count,
            full_count: cands.iter().filter(|c| c.full).count(),
            books: book_seen.iter().filter(|&&b| b).count(),
            ids: cands.iter().map(|c| c.first).collect(),
            spans: cands.iter().map(|c| c.span).collect(),
            scores: cands.iter().map(|c| c.score).collect(),
            full: cands.iter().map(|c| c.full).collect(),
            hits,
            targets,
            query,
        }
    }

    fn reference_results(&self, query: Query, r: Reference) -> Results {
        let mut ids = Vec::new();
        let (from, to) = r.verses.unwrap_or((1, 999));
        if let Some(c) = r.chapter {
            for v in from..=to {
                match self.find_verse(r.book, c, v) {
                    Some(id) => ids.push(id as u32),
                    None => break,
                }
            }
        }
        let mut hits = vec![0u8; self.len()];
        for &v in &ids {
            hits[v as usize] = 255;
        }
        Results {
            literal_count: ids.len(),
            full_count: ids.len(),
            books: (!ids.is_empty()) as usize,
            scores: vec![1.0; ids.len()],
            full: vec![true; ids.len()],
            spans: vec![1; ids.len()],
            ids,
            hits,
            targets: HashMap::new(),
            query,
        }
    }
}

/// How one word of a verse matched the query, if it did.
pub struct Mark {
    pub term: usize,
    pub why: Why,
    pub strongs: Option<u16>,
}

impl Results {
    pub fn mark(&self, bible: &Bible, word: &str, word_index: usize) -> Option<Mark> {
        let mut best: Option<(f32, Mark)> = None;
        let mut consider = |w: f32, m: Mark| {
            if best.as_ref().is_none_or(|(bw, _)| w > *bw) {
                best = Some((w, m));
            }
        };
        if let Some(s) = bible.stem_id(word) {
            if let Some(&(term, w, why)) = self.targets.get(&Target::Stem(s)) {
                let why = match why {
                    _ if self.query.terms[term].stop => Why::Stop,
                    Why::Exact if text::normalize(word) != self.query.terms[term].typed => Why::Form,
                    other => other,
                };
                consider(w, Mark { term, why, strongs: None });
            }
        }
        for &code in bible.strongs_of_word(word_index) {
            if let Some(&(term, w, why)) = self.targets.get(&Target::Strongs(code)) {
                consider(w * 0.99, Mark { term, why, strongs: Some(code).filter(|_| why == Why::Strongs) });
            }
        }
        best.map(|(_, m)| m)
    }

    /// Short reasons for one verse: "tired ≈ weary", "love → charity (G26 agapē)",
    /// each with the Strong's number behind the matched word when it has one.
    pub fn reasons(&self, bible: &Bible, v: usize) -> Vec<(String, Option<u16>)> {
        let mut out: Vec<(String, Option<u16>)> = Vec::new();
        let text = bible.verse_text(v);
        for (k, (a, b)) in text::words(text).into_iter().enumerate() {
            let word = &text[a..b];
            let w = bible.word_off[v] as usize + k;
            let Some(m) = self.mark(bible, word, w) else { continue };
            let typed = &self.query.terms[m.term].typed;
            let shown = text::normalize(word);
            let r = match m.why {
                Why::Exact | Why::Stop => continue,
                Why::Form | Why::Prefix => format!("{typed} → {shown}"),
                Why::Typo => format!("{typed} → {shown} (spelling)"),
                Why::Synonym => format!("{typed} ≈ {shown}"),
                Why::Strongs => {
                    let code = m.strongs.unwrap_or(0);
                    let tl = bible.entry(code).map(|e| e.translit.as_str()).unwrap_or("");
                    if shown == *typed {
                        continue;
                    }
                    format!("{typed} → {shown} ({} {tl})", strongs_name(code))
                }
            };
            if !out.iter().any(|(text, _)| *text == r) {
                out.push((r, m.strongs.or_else(|| bible.strongs_of_word(w).first().copied())));
            }
        }
        out.truncate(4);
        out
    }
}
