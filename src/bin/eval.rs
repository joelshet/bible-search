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

/// Passages remembered across verses. Each answer is a set of verses that must all sit in one
/// result; a query with two homes (Matthew and Luke) lists both. The run scoring was tuned on these.
const PASSAGES: &[(&str, &[&[&str]])] = &[
    ("trust in the lord with all your heart and lean not on your own understanding in all your ways acknowledge him and he will make your paths straight", &[&["PRO 3:5", "PRO 3:6"]]),
    ("the lord is my shepherd i shall not want he makes me lie down in green pastures he leads me beside still waters", &[&["PSA 23:1", "PSA 23:2"]]),
    ("ask and it will be given to you seek and you will find knock and the door will be opened for everyone who asks receives", &[&["MAT 7:7", "MAT 7:8"], &["LUK 11:9", "LUK 11:10"]]),
    ("i lift up my eyes to the hills where does my help come from my help comes from the lord", &[&["PSA 121:1", "PSA 121:2"]]),
    ("do not be anxious about anything but in everything by prayer and the peace of god which passes all understanding will guard your hearts", &[&["PHP 4:6", "PHP 4:7"]]),
    ("fruit of the spirit is love joy peace patience kindness goodness faithfulness gentleness self control", &[&["GAL 5:22", "GAL 5:23"]]),
    ("for god so loved the world for god did not send his son into the world to condemn the world", &[&["JHN 3:16", "JHN 3:17"]]),
    ("blessed are the poor in spirit blessed are those who mourn blessed are the meek", &[&["MAT 5:3", "MAT 5:4", "MAT 5:5"]]),
    ("shepherd green pastures still waters valley of the shadow of death", &[&["PSA 23:2", "PSA 23:4"]]),
    ("love is patient love is kind it does not envy it is not self seeking it is not easily angered", &[&["1CO 13:4", "1CO 13:5"]]),
    ("armor of god belt of truth breastplate of righteousness shield of faith sword of the spirit", &[&["EPH 6:14", "EPH 6:16", "EPH 6:17"]]),
    ("valley of dry bones prophesy breath", &[&["EZK 37:4"]]),
];

/// Written before the run scoring was tuned, scored once after, and never tuned against.
const PASSAGES_HELD_OUT: &[(&str, &[&[&str]])] = &[
    ("in the beginning was the word and the word was with god and the word was god he was with god in the beginning", &[&["JHN 1:1", "JHN 1:2"]]),
    ("our father in heaven hallowed be your name your kingdom come your will be done on earth as it is in heaven", &[&["MAT 6:9", "MAT 6:10"], &["LUK 11:2"]]),
    ("give us this day our daily bread and forgive us our debts as we forgive our debtors", &[&["MAT 6:11", "MAT 6:12"]]),
    ("the lord bless you and keep you the lord make his face shine upon you and be gracious to you", &[&["NUM 6:24", "NUM 6:25"]]),
    ("go and make disciples of all nations baptizing them in the name of the father and of the son and of the holy spirit teaching them to obey everything i have commanded you", &[&["MAT 28:19", "MAT 28:20"]]),
    ("for by grace you have been saved through faith and this is not your own doing it is the gift of god not a result of works so that no one may boast", &[&["EPH 2:8", "EPH 2:9"]]),
    ("love the lord your god with all your heart and with all your soul and with all your mind this is the first and greatest commandment", &[&["MAT 22:37", "MAT 22:38"]]),
    ("a time to be born and a time to die a time to plant a time to kill and a time to heal a time to weep and a time to laugh", &[&["ECC 3:2", "ECC 3:3", "ECC 3:4"]]),
    ("even though i walk through the valley of the shadow of death i will fear no evil you prepare a table before me in the presence of my enemies", &[&["PSA 23:4", "PSA 23:5"]]),
    ("have you not known have you not heard the everlasting god does not faint or grow weary he gives power to the faint", &[&["ISA 40:28", "ISA 40:29"]]),
    ("consider the lilies of the field how they grow they toil not neither do they spin yet solomon in all his glory was not arrayed like one of these", &[&["MAT 6:28", "MAT 6:29"], &["LUK 12:27"]]),
    ("rejoice in the lord always again i say rejoice let your gentleness be known to all the lord is near", &[&["PHP 4:4", "PHP 4:5"]]),
    ("the heavens declare the glory of god the skies proclaim the work of his hands day after day they pour forth speech", &[&["PSA 19:1", "PSA 19:2"]]),
    ("create in me a clean heart o god and renew a right spirit within me do not cast me away from your presence", &[&["PSA 51:10", "PSA 51:11"]]),
    ("i am the vine you are the branches apart from me you can do nothing if anyone does not abide in me he is thrown away", &[&["JHN 15:5", "JHN 15:6"]]),
    ("come to me all who are weary and burdened and i will give you rest take my yoke upon you and learn from me", &[&["MAT 11:28", "MAT 11:29"]]),
];

const DEMOS: &[&str] = &["nebuchadnezer", "agape", "G26", "jn 3:16", "love in:john", "\"the lord is my shepherd\""];


fn find(b: &Bible, r: &str) -> usize {
    let (code, cv) = r.split_once(' ').unwrap();
    let (c, v) = cv.split_once(':').unwrap();
    b.find_verse(books::by_code(code).unwrap(), c.parse().unwrap(), v.parse().unwrap()).unwrap()
}

/// "Psalms 23:1-4" for a run of verses, "Psalms 23:1" for one.
fn label(b: &Bible, r: &bible::search::Results, i: usize) -> String {
    let v = r.ids[i] as usize;
    match r.spans[i] {
        1 => b.reference(v),
        n => format!("{}-{}", b.reference(v), b.verse[v + n as usize - 1]),
    }
}

/// The first result that holds every verse of one of the answers.
fn rank(r: &bible::search::Results, answers: &[Vec<usize>]) -> Option<usize> {
    (0..r.ids.len()).find(|&i| {
        let range = r.ids[i] as usize..r.ids[i] as usize + r.spans[i] as usize;
        answers.iter().any(|a| a.iter().all(|v| range.contains(v)))
    })
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
            println!("{:2}. {:7.2} {}", i + 1, r.scores[i], label(&b, &r, i));
            for v in v as usize..v as usize + r.spans[i] as usize {
                println!("      {}\n      {:?}", b.verse_text(v), r.reasons(&b, v).iter().map(|(text, _)| text.as_str()).collect::<Vec<_>>());
            }
        }
        return;
    }

    let mut worst = 0f64;
    type Case = (&'static str, Vec<Vec<&'static str>>);
    let single = |set: &[(&'static str, &[&'static str])]| -> Vec<Case> { set.iter().map(|(q, want)| (*q, want.iter().map(|w| vec![*w]).collect())).collect() };
    let several = |set: &[(&'static str, &[&[&'static str]])]| -> Vec<Case> { set.iter().map(|(q, want)| (*q, want.iter().map(|a| a.to_vec()).collect())).collect() };
    let sets = [("tuned set", single(CASES)), ("held-out set", single(HELD_OUT)), ("passages, tuned", several(PASSAGES)), ("passages, held out", several(PASSAGES_HELD_OUT))];
    for (name, cases) in &sets {
        let (mut top1, mut top3) = (0, 0);
        println!("\n{name}");
        for (q, want) in cases {
            let t = Instant::now();
            let r = b.search(q, false, false);
            let us = t.elapsed().as_secs_f64() * 1e6;
            worst = worst.max(us);
            let answers: Vec<Vec<usize>> = want.iter().map(|a| a.iter().map(|w| find(&b, w)).collect()).collect();
            let rank = rank(&r, &answers);
            top1 += (rank == Some(0)) as usize;
            top3 += rank.is_some_and(|r| r < 3) as usize;
            let shown = rank.map_or("miss".to_string(), |r| format!("#{}", r + 1));
            let first = if r.ids.is_empty() { String::new() } else { label(&b, &r, 0) };
            let q: String = q.chars().take(58).collect();
            println!("{shown:>5} {us:6.0} µs  {q:58} top: {first}");
        }
        println!("{name}: top 1 {top1}/{n}, top 3 {top3}/{n}", n = cases.len());
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
