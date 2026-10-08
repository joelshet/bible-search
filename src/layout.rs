//! The map: an ordered treemap where every verse's area is proportional
//! to its length. Text can then be drawn at one size everywhere, and books and
//! chapters stay in reading order, Genesis top-left to Revelation bottom-right.

use crate::index::Bible;

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Lay `weights` out in order inside `r`: split the run where its weight halves,
/// cut the rectangle across its longer side in that proportion, and recurse.
/// Near-square chapters come out as two columns of verses, like a printed page.
pub fn split(weights: &[f64], r: Rect) -> Vec<Rect> {
    let mut out = vec![r; weights.len()];
    let mut prefix = vec![0.0; weights.len() + 1];
    for (i, w) in weights.iter().enumerate() {
        prefix[i + 1] = prefix[i] + w;
    }
    place(&prefix, 0, weights.len(), r, &mut out);
    out
}

fn place(prefix: &[f64], a: usize, b: usize, r: Rect, out: &mut [Rect]) {
    if b - a == 1 {
        out[a] = r;
        return;
    }
    let total = prefix[b] - prefix[a];
    let half = prefix[a] + total / 2.0;
    // first index whose running total passes the halfway mark, kept inside (a, b)
    let mut k = a + 1 + prefix[a + 1..b].partition_point(|&p| p < half);
    if k > a + 1 && (prefix[k - 1] - half).abs() < (prefix[k] - half).abs() {
        k -= 1;
    }
    let k = k.clamp(a + 1, b - 1);
    let f = (prefix[k] - prefix[a]) / total;
    let (r1, r2) = if r.w >= r.h {
        (Rect { w: r.w * f, ..r }, Rect { x: r.x + r.w * f, w: r.w * (1.0 - f), ..r })
    } else {
        (Rect { h: r.h * f, ..r }, Rect { y: r.y + r.h * f, h: r.h * (1.0 - f), ..r })
    };
    place(prefix, a, k, r1, out);
    place(prefix, k, b, r2, out);
}

/// Sections of the canon as book index ranges; each becomes a region of the map.
const OLD: &[std::ops::Range<usize>] = &[0..5, 5..17, 17..22, 22..39]; // law, history, poetry, prophets
const NEW: &[std::ops::Range<usize>] = &[39..44, 44..57, 57..66]; // gospels and Acts, Paul, the rest

/// [books × 4][chapters × 4][verses × 4] as x, y, w, h in an `aspect` × 1 rectangle.
/// Old Testament across the top, New Testament along the bottom.
pub fn layout(b: &Bible, aspect: f64) -> Vec<f32> {
    let n = b.len();
    let weight: Vec<f64> = (0..n).map(|v| b.verse_text(v).len() as f64 + 30.0).collect();
    let chapters = b.chapter_start.len();
    let chapter_weight: Vec<f64> = (0..chapters).map(|c| b.chapter_range(c).map(|v| weight[v]).sum()).collect();
    let mut book_chapters: Vec<Vec<usize>> = vec![vec![]; 66];
    for c in 0..chapters {
        book_chapters[b.book[b.chapter_start[c] as usize] as usize].push(c);
    }
    let book_weight: Vec<f64> = book_chapters.iter().map(|cs| cs.iter().map(|&c| chapter_weight[c]).sum()).collect();
    let section_weight = |s: &std::ops::Range<usize>| book_weight[s.clone()].iter().sum::<f64>();

    let ot: f64 = OLD.iter().map(section_weight).sum();
    let nt: f64 = NEW.iter().map(section_weight).sum();
    let split_at = ot / (ot + nt);
    let testaments = [
        (OLD, Rect { x: 0.0, y: 0.0, w: aspect, h: split_at }),
        (NEW, Rect { x: 0.0, y: split_at, w: aspect, h: 1.0 - split_at }),
    ];
    let mut book_rects = vec![Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 }; 66];
    for (sections, r) in testaments {
        let ws: Vec<f64> = sections.iter().map(section_weight).collect();
        for (sec, sr) in sections.iter().zip(split(&ws, r)) {
            for (bk, br) in sec.clone().zip(split(&book_weight[sec.clone()], sr)) {
                book_rects[bk] = br;
            }
        }
    }
    let mut chapter_rects = vec![Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 }; chapters];
    let mut verse_rects = vec![Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 }; n];
    for (bk, cs) in book_chapters.iter().enumerate() {
        let ws: Vec<f64> = cs.iter().map(|&c| chapter_weight[c]).collect();
        for (&c, r) in cs.iter().zip(split(&ws, book_rects[bk])) {
            chapter_rects[c] = r;
            let range = b.chapter_range(c);
            for (v, vr) in range.clone().zip(split(&weight[range], r)) {
                verse_rects[v] = vr;
            }
        }
    }
    book_rects
        .iter()
        .chain(&chapter_rects)
        .chain(&verse_rects)
        .flat_map(|r| [r.x as f32, r.y as f32, r.w as f32, r.h as f32])
        .collect()
}
