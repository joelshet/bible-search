//! Half-remembered queries and where the verse people meant lands.
//!
//!   cargo run --release --bin eval -- web/data/bible.idx [query...]
//!
//! With no query, runs the fixed set and prints each target's rank. With a
//! query, prints the top ten results and why they matched.

use bible::books;
use bible::index::Bible;
use std::time::Instant;
use std::{env, fs, process};

/// (what someone might type, verses that count as the right answer)
const CASES: &[(&str, &[&str])] = &[
    ("eagles not getting tired", &["ISA 40:31"]),
    ("god so loved the world", &["JHN 3:16"]),
    ("love is patient love is kind", &["1CO 13:4"]),
    ("the lord is my shepherd", &["PSA 23:1"]),
    ("i can do all things through christ", &["PHP 4:13"]),
    ("be still and know that i am god", &["PSA 46:10"]),
    ("faith hope and love the greatest of these is love", &["1CO 13:13"]),
    ("plans to prosper you and not to harm you", &["JER 29:11"]),
    ("trust in the lord with all your heart", &["PRO 3:5"]),
    ("jesus wept", &["JHN 11:35"]),
    ("in my fathers house are many rooms", &["JHN 14:2"]),
    ("do not worry about tomorrow", &["MAT 6:34"]),
    ("wages of sin is death", &["ROM 6:23"]),
    ("all things work together for good", &["ROM 8:28"]),
    ("fruit of the spirit love joy peace", &["GAL 5:22"]),
    ("a time for everything under heaven", &["ECC 3:1"]),
    ("the truth will set you free", &["JHN 8:32"]),
    ("be strong and courageous", &["JOS 1:9", "JOS 1:6", "JOS 1:7", "DEU 31:6", "DEU 31:7", "JOS 1:18", "JOS 10:25", "1CH 22:13", "1CH 28:20", "2CH 32:7", "DEU 31:23"]),
    ("pride comes before a fall", &["PRO 16:18"]),
    ("nothing can separate us from the love of god", &["ROM 8:39", "ROM 8:35"]),
    ("iron sharpens iron", &["PRO 27:17"]),
    ("cast your cares on him because he cares for you", &["1PE 5:7"]),
    ("the joy of the lord is your strength", &["NEH 8:10"]),
    ("the battle belongs to the lord", &["1SA 17:47", "2CH 20:15"]),
    ("do not be anxious about anything", &["PHP 4:6"]),
    ("weeping lasts for the night but joy comes in the morning", &["PSA 30:5"]),
    ("a gentle answer turns away anger", &["PRO 15:1"]),
    ("camel through the eye of a needle", &["MAT 19:24", "MRK 10:25", "LUK 18:25"]),
    ("love your neighbor as yourself", &["LEV 19:18", "MAT 19:19", "MAT 22:39", "MRK 12:31", "ROM 13:9", "GAL 5:14", "JAS 2:8", "LUK 10:27", "MRK 12:33"]),
];

/// Written after the ranking and the modern-word list were tuned, and never tuned against.
const HELD_OUT: &[(&str, &[&str])] = &[
    ("the lord bless you and keep you", &["NUM 6:24"]),
    ("be kind to one another tenderhearted forgiving", &["EPH 4:32"]),
    ("where two or three gather in my name", &["MAT 18:20"]),
    ("his mercies are new every morning", &["LAM 3:23", "LAM 3:22"]),
    ("do to others what you would have them do to you", &["MAT 7:12", "LUK 6:31"]),
    ("be angry and do not sin", &["EPH 4:26"]),
    ("your word is a lamp to my feet", &["PSA 119:105"]),
    ("my grace is enough for you", &["2CO 12:9"]),
    ("seek first his kingdom", &["MAT 6:33"]),
    ("train a child in the way he should go", &["PRO 22:6"]),
    ("god loves a cheerful giver", &["2CO 9:7"]),
    ("money is the root of all evil", &["1TI 6:10"]),
    ("dont let the sun go down while you are still angry", &["EPH 4:26"]),
    ("come to me all who are weary and burdened", &["MAT 11:28"]),
    ("the lord will fight for you you only need to be still", &["EXO 14:14"]),
    ("with god all things are possible", &["MAT 19:26", "MRK 10:27"]),
    ("the meek will inherit the earth", &["MAT 5:5"]),
    ("god is love", &["1JN 4:8", "1JN 4:16"]),
    ("turn the other cheek", &["MAT 5:39", "LUK 6:29"]),
    ("carry each others burdens", &["GAL 6:2"]),
    ("the spirit is willing but the body is weak", &["MAT 26:41", "MRK 14:38"]),
    ("what good is it to gain the whole world but lose your soul", &["MAT 16:26", "MRK 8:36", "LUK 9:25"]),
    ("everyone has sinned and fallen short of gods glory", &["ROM 3:23"]),
    ("faith without deeds is dead", &["JAS 2:20", "JAS 2:26", "JAS 2:17"]),
    ("let your light shine before people", &["MAT 5:16"]),
];

const DEMOS: &[&str] = &["nebuchadnezer", "agape", "G26", "jn 3:16", "love in:john", "\"the lord is my shepherd\""];


fn find(b: &Bible, r: &str) -> usize {
    let (code, cv) = r.split_once(' ').unwrap();
    let (c, v) = cv.split_once(':').unwrap();
    b.find_verse(books::by_code(code).unwrap(), c.parse().unwrap(), v.parse().unwrap()).unwrap()
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let Some(path) = args.get(1) else {
        eprintln!("usage: eval INDEX [query...]");
        process::exit(2);
    };
    let t = Instant::now();
    let b = Bible::decode(&fs::read(path).unwrap_or_else(|e| {
        eprintln!("{path}: {e}");
        process::exit(1);
    }))
    .unwrap();
    eprintln!("loaded in {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);

    if args.len() > 2 {
        let q = args[2..].join(" ");
        let t = Instant::now();
        let r = b.search(&q, false, false);
        let us = t.elapsed().as_secs_f64() * 1e6;
        println!("{} results ({} with every word) in {:.0} µs  notes: {:?}", r.ids.len(), r.full_count, us, r.query.notes);
        for t in &r.query.terms {
            let exps: Vec<String> = t
                .expansions
                .iter()
                .take(12)
                .map(|e| format!("{:?}:{:.2}", e.target, e.weight).replace("Stem(", "s(").replace("Strongs(", "g("))
                .collect();
            println!("  term {:?} stop={} {}", t.typed, t.stop, exps.join(" "));
        }
        for (i, &v) in r.ids.iter().take(10).enumerate() {
            let v = v as usize;
            println!("{:2}. {:7.2} {}  {}\n      {:?}", i + 1, r.scores[i], b.reference(v), b.verse_text(v), r.reasons(&b, v));
        }
        return;
    }

    let mut worst = 0f64;
    for (label, cases) in [("tuned set", CASES), ("held-out set", HELD_OUT)] {
        let (mut top1, mut top3) = (0, 0);
        println!("\n{label}");
        for (q, want) in cases.iter().filter(|(_, w)| !w.is_empty()) {
            let t = Instant::now();
            let r = b.search(q, false, false);
            let us = t.elapsed().as_secs_f64() * 1e6;
            worst = worst.max(us);
            let ids: Vec<usize> = want.iter().map(|w| find(&b, w)).collect();
            let rank = r.ids.iter().position(|&v| ids.contains(&(v as usize)));
            top1 += (rank == Some(0)) as usize;
            top3 += rank.is_some_and(|r| r < 3) as usize;
            let shown = rank.map_or("miss".to_string(), |r| format!("#{}", r + 1));
            let first = r.ids.first().map(|&v| b.reference(v as usize)).unwrap_or_default();
            println!("{shown:>5} {us:6.0} µs  {q:58} top: {first}");
        }
        let judged = cases.iter().filter(|(_, w)| !w.is_empty()).count();
        println!("{label}: top 1 {top1}/{judged}, top 3 {top3}/{judged}");
    }
    println!();
    for q in DEMOS {
        let t = Instant::now();
        let r = b.search(q, false, false);
        let us = t.elapsed().as_secs_f64() * 1e6;
        let first = r.ids.first().map(|&v| b.reference(v as usize)).unwrap_or_default();
        println!("   -  {us:6.0} µs  {q:30} {} results → {first} {:?}", r.ids.len(), r.query.notes);
    }
    println!("slowest judged query: {worst:.0} µs");
}
