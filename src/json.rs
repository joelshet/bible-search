//! JSON the page reads. Hand-written to keep the wasm small and dependency-free.

use crate::books::BOOKS;
use crate::index::{strongs_code, strongs_name, Bible};
use crate::search::Results;
use crate::text;
use std::fmt::Write;

pub fn esc(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A verse as segments: ["gap text"] or ["word", flags, "why", "strongs codes", term].
/// With results, "why" lists each reason as ["text", "strongs code or empty"].
pub fn verse(out: &mut String, b: &Bible, v: usize, results: Option<&Results>) {
    let text = b.verse_text(v);
    let _ = write!(out, "{{\"id\":{v},\"ref\":");
    esc(out, &b.reference(v));
    let _ = write!(out, ",\"b\":{},\"c\":{},\"v\":{},\"seg\":[", b.book[v], b.chapter[v], b.verse[v]);
    let mut at = 0;
    let base = b.word_off[v] as usize;
    for (k, (s, e)) in text::words(text).into_iter().enumerate() {
        if s > at {
            out.push('[');
            esc(out, &text[at..s]);
            out.push_str("],");
        }
        let w = base + k;
        out.push('[');
        esc(out, &text[s..e]);
        let codes: Vec<String> = b.strongs_of_word(w).iter().map(|&c| strongs_name(c)).collect();
        let mark = results.and_then(|r| r.mark(b, &text[s..e], w));
        let _ = write!(out, ",{},", b.word_flags[w]);
        esc(out, mark.as_ref().map_or("", |m| m.why.code()));
        out.push(',');
        esc(out, &codes.join(" "));
        let _ = write!(out, ",{}],", mark.map_or(-1, |m| m.term as i64));
        at = e;
    }
    if at < text.len() {
        out.push('[');
        esc(out, &text[at..]);
        out.push_str("],");
    }
    if out.ends_with(',') {
        out.pop();
    }
    out.push(']');
    if let Some(r) = results {
        out.push_str(",\"why\":[");
        for (i, (reason, code)) in r.reasons(b, v).iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push('[');
            esc(out, reason);
            out.push(',');
            esc(out, &code.map(strongs_name).unwrap_or_default());
            out.push(']');
        }
        out.push(']');
    }
    out.push('}');
}

pub fn summary(b: &Bible, r: &Results, micros: f64) -> String {
    let mut out = String::new();
    let _ = write!(
        out,
        "{{\"total\":{},\"full\":{},\"literal\":{},\"books\":{},\"terms\":{},\"micros\":{micros:.1},\"reference\":{},\"word\":",
        r.ids.len(),
        r.full_count,
        r.literal_count,
        r.books,
        r.query.terms.iter().filter(|t| !t.stop).count(),
        r.query.reference.is_some()
    );
    let content: Vec<&str> = r.query.terms.iter().filter(|t| !t.stop).map(|t| t.typed.as_str()).collect();
    esc(&mut out, &content.join(" "));
    out.push_str(",\"notes\":[");
    for (i, n) in r.query.notes.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        esc(&mut out, n);
    }
    out.push_str("],\"book\":");
    match r.query.book_hint {
        Some(i) => {
            let first = (0..b.len()).find(|&v| b.book[v] as usize == i).unwrap_or(0);
            let _ = write!(out, "{{\"name\":\"{}\",\"first\":{first}}}", BOOKS[i].name);
        }
        None => out.push_str("null"),
    }
    out.push('}');
    out
}

pub fn page(b: &Bible, r: &Results, offset: usize, limit: usize) -> String {
    let mut out = String::from("[");
    for (i, &v) in r.ids.iter().skip(offset).take(limit).enumerate() {
        if i > 0 {
            out.push(',');
        }
        verse(&mut out, b, v as usize, Some(r));
        out.pop();
        let _ = write!(out, ",\"full\":{}}}", r.full[offset + i]);
    }
    out.push(']');
    out
}

pub fn chapter(b: &Bible, v: usize, results: Option<&Results>) -> String {
    let c = b.chapter_of(v);
    let range = b.chapter_range(c);
    let mut out = String::new();
    let _ = write!(out, "{{\"chapter\":{c},\"name\":");
    esc(&mut out, &format!("{} {}", BOOKS[b.book[v] as usize].name, b.chapter[v]));
    out.push_str(",\"title\":");
    match b.titles.iter().find(|(s, _)| *s as usize == range.start) {
        Some((_, t)) => esc(&mut out, t),
        None => out.push_str("null"),
    }
    let _ = write!(out, ",\"prev\":{},\"next\":{},\"verses\":[", c.checked_sub(1).map_or(-1, |p| b.chapter_start[p] as i64), b.chapter_start.get(c + 1).map_or(-1, |&n| n as i64));
    for (i, v) in range.enumerate() {
        if i > 0 {
            out.push(',');
        }
        verse(&mut out, b, v, results);
    }
    out.push_str("]}");
    out
}

pub fn strongs(b: &Bible, name: &str) -> String {
    let Some(code) = strongs_code(name) else { return "null".into() };
    let Some(e) = b.entry(code) else { return "null".into() };
    let mut out = String::new();
    out.push_str("{\"code\":");
    esc(&mut out, &strongs_name(code));
    for (k, v) in [
        ("lemma", &e.lemma),
        ("translit", &e.translit),
        ("pron", &e.pron),
        ("derivation", &e.derivation),
        ("definition", &e.definition),
        ("usage", &e.kjv_usage),
    ] {
        let _ = write!(out, ",\"{k}\":");
        esc(&mut out, v);
    }
    let (verses, tfs) = b.strongs_postings.get(code as usize);
    let _ = write!(
        out,
        ",\"verses\":{},\"uses\":{},\"lang\":\"{}\",\"renderings\":[",
        verses.len(),
        tfs.iter().map(|&t| t as u32).sum::<u32>(),
        if code > 10000 { "greek" } else { "hebrew" }
    );
    for (i, &(s, n)) in b.renderings[code as usize].iter().take(16).enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('[');
        esc(&mut out, &b.surface[s as usize]);
        let _ = write!(out, ",{n}]");
    }
    out.push_str("]}");
    out
}

/// Books as [code, name, short name], chapter starts, and every verse's text, for the map and present mode.
pub fn meta(b: &Bible) -> String {
    let mut out = String::from("{\"books\":[");
    for (i, book) in BOOKS.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "[\"{}\",", book.code);
        esc(&mut out, book.name);
        out.push(',');
        esc(&mut out, book.short);
        out.push(']');
    }
    out.push_str("],\"chapterStart\":[");
    for (i, s) in b.chapter_start.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "{s}");
    }
    out.push_str("],\"chapterBook\":[");
    for (i, &s) in b.chapter_start.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "{}", b.book[s as usize]);
    }
    out.push_str("],\"titles\":{");
    for (i, (v, t)) in b.titles.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "\"{v}\":");
        esc(&mut out, t);
    }
    out.push_str("},\"text\":");
    let mut all = String::with_capacity(b.text.len() + b.len());
    for v in 0..b.len() {
        if v > 0 {
            all.push('\n');
        }
        all.push_str(b.verse_text(v));
    }
    esc(&mut out, &all);
    out.push('}');
    out
}
