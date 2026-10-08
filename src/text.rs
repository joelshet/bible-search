//! Tokenizing and stemming shared by the index builder and the query side.
//! Both must agree exactly, so this is the only place words get normalized.

/// Byte range of each word in `s`. A word is a run of letters, joined across
/// internal apostrophes and hyphens ("Peter's", "Beth-el").
pub fn words(s: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    let mut last_letter_end = 0;
    let mut iter = s.char_indices().peekable();
    while let Some((i, c)) = iter.next() {
        if c.is_alphabetic() {
            if start.is_none() {
                start = Some(i);
            }
            last_letter_end = i + c.len_utf8();
        } else if start.is_some() && matches!(c, '\'' | '’' | '-') {
            if !iter.peek().is_some_and(|&(_, n)| n.is_alphabetic()) {
                out.push((start.take().unwrap(), last_letter_end));
            }
        } else if let Some(st) = start.take() {
            out.push((st, last_letter_end));
        }
    }
    if let Some(st) = start {
        out.push((st, last_letter_end));
    }
    out
}

/// Lowercase, drop a possessive 's, drop apostrophes and hyphens, fold accents.
pub fn normalize(word: &str) -> String {
    let mut out = String::with_capacity(word.len());
    normalize_into(word, &mut out);
    out
}

/// `normalize` into a reused buffer, for hot loops.
pub fn normalize_into(word: &str, out: &mut String) {
    out.clear();
    let word = word
        .strip_suffix("'s")
        .or_else(|| word.strip_suffix("’s"))
        .or_else(|| word.strip_suffix("'S"))
        .or_else(|| word.strip_suffix("’S"))
        .unwrap_or(word);
    for c in word.chars() {
        if c.is_ascii() {
            if c.is_ascii_alphanumeric() {
                out.push(c.to_ascii_lowercase());
            }
        } else {
            out.extend(c.to_lowercase().filter_map(fold));
        }
    }
}

fn fold(c: char) -> Option<char> {
    Some(match c {
        'a'..='z' => c,
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'ç' | 'ć' | 'č' => 'c',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'ĭ' | 'į' => 'i',
        'ñ' | 'ń' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ō' | 'ŏ' | 'ő' => 'o',
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ŭ' | 'ů' => 'u',
        'ý' | 'ÿ' => 'y',
        'ṭ' => 't',
        'ş' | 'š' | 'ś' | 'ṣ' => 's',
        'ḥ' | 'ẖ' => 'h',
        'ẓ' | 'ž' => 'z',
        'æ' => 'a',
        c if c.is_ascii_digit() => c,
        _ => return None,
    })
}

/// Irregular and archaic forms mapped to the form the suffix rules expect.
const IRREGULAR: &[(&str, &str)] = &[
    ("hath", "have"), ("hast", "have"), ("has", "have"), ("had", "have"), ("hadst", "have"), ("having", "have"), ("goes", "go"), ("goeth", "go"), ("goest", "go"), ("doeth", "do"), ("doest", "do"),
    ("doth", "do"), ("dost", "do"), ("does", "do"), ("did", "do"), ("didst", "do"), ("done", "do"),
    ("saith", "say"), ("said", "say"), ("saidst", "say"), ("says", "say"),
    ("spake", "speak"), ("spoke", "speak"), ("spoken", "speak"), ("spakest", "speak"),
    ("art", "be"), ("wast", "be"), ("wert", "be"), ("is", "be"), ("am", "be"), ("are", "be"),
    ("was", "be"), ("were", "be"), ("been", "be"), ("being", "be"),
    ("shalt", "shall"), ("wilt", "will"), ("canst", "can"), ("couldest", "could"), ("mayest", "may"),
    ("ran", "run"), ("came", "come"), ("went", "go"), ("gone", "go"), ("knew", "know"), ("known", "know"),
    ("gave", "give"), ("given", "give"), ("took", "take"), ("taken", "take"), ("saw", "see"), ("seen", "see"),
    ("ate", "eat"), ("eaten", "eat"), ("brought", "bring"), ("taught", "teach"), ("sought", "seek"),
    ("bought", "buy"), ("fought", "fight"), ("caught", "catch"), ("built", "build"), ("sent", "send"),
    ("slew", "slay"), ("slain", "slay"), ("smote", "smite"), ("smitten", "smite"), ("arose", "arise"),
    ("arisen", "arise"), ("rose", "rise"), ("risen", "rise"), ("begat", "beget"), ("begotten", "beget"),
    ("forgave", "forgive"), ("forgiven", "forgive"), ("wrote", "write"), ("written", "write"),
    ("drank", "drink"), ("drunk", "drink"), ("sang", "sing"), ("sung", "sing"), ("chose", "choose"),
    ("chosen", "choose"), ("fell", "fall"), ("fallen", "fall"), ("held", "hold"), ("kept", "keep"),
    ("met", "meet"), ("sat", "sit"), ("stood", "stand"), ("told", "tell"), ("wept", "weep"),
    ("slept", "sleep"), ("bare", "bear"), ("bore", "bear"), ("born", "bear"), ("borne", "bear"),
    ("drew", "draw"), ("drawn", "draw"), ("grew", "grow"), ("grown", "grow"), ("threw", "throw"),
    ("thrown", "throw"), ("shook", "shake"), ("shaken", "shake"), ("forsook", "forsake"),
    ("forsaken", "forsake"), ("abode", "abide"), ("strove", "strive"), ("clave", "cleave"),
    ("trodden", "tread"), ("trode", "tread"), ("men", "man"), ("women", "woman"), ("children", "child"),
    ("brethren", "brother"), ("feet", "foot"), ("teeth", "tooth"), ("oxen", "ox"), ("lice", "louse"),
    ("thee", "you"), ("thou", "you"), ("ye", "you"), ("thy", "your"), ("thine", "your"),
    ("yourselves", "yourself"), ("got", "get"), ("gotten", "get"), ("felt", "feel"), ("fed", "feed"),
    ("fled", "flee"), ("led", "lead"), ("laid", "lay"), ("lain", "lie"), ("lay", "lay"), ("paid", "pay"),
    ("heard", "hear"), ("made", "make"), ("found", "find"), ("stole", "steal"), ("stolen", "steal"),
    ("rode", "ride"), ("ridden", "ride"), ("hid", "hide"), ("hidden", "hide"), ("bound", "bind"),
    ("wound", "wind"), ("ground", "ground"), ("thought", "think"), ("lost", "lose"), ("left", "leave"),
    ("blew", "blow"), ("flew", "fly"), ("knelt", "kneel"), ("dwelt", "dwell"), ("meant", "mean"),
    ("worn", "wear"), ("wore", "wear"), ("tare", "tear"), ("torn", "tear"), ("swore", "swear"),
    ("sware", "swear"), ("sworn", "swear"), ("began", "begin"), ("begun", "begin"), ("overcame", "overcome"),
    ("understood", "understand"), ("withstood", "withstand"), ("became", "become"), ("better", "better"),
];

/// Words the suffix rules would mangle into a different word.
const KEEP: &[&str] = &[
    "forest", "rest", "honest", "harvest", "west", "nest", "chest", "guest", "priest", "beast", "feast",
    "least", "breast", "jest", "manifest", "tempest", "earnest", "interest", "request", "conquest",
    "behest", "unrest", "midst", "east", "lest", "best", "highest", "thing", "nothing", "something",
    "anything", "everything", "spring", "string", "evening", "king", "wing", "ring", "sling", "bring",
    "sing", "swing", "sting", "ceiling", "wilderness", "witness", "harness", "seed", "need", "speed",
    "creed", "indeed", "deed", "feed", "bleed", "breed", "greed", "weed", "reed", "hundred", "kindred",
    "sacred", "naked", "wicked", "red", "bed", "shed", "wed", "led", "fled",
    "bread", "head", "dead", "lead", "thread", "dread", "tread", "spread", "abroad", "ahead", "instead",
    "always", "alas", "bless", "less", "unless", "guess", "press", "dress", "goodness", "kindness",
    "seth", "beth", "japheth", "nazareth", "elisabeth", "heth", "teeth", "lord", "news", "this", "his",
    "was", "is", "has", "us", "thus", "jesus", "moses", "amos", "lazarus", "cyrus", "darius", "judas",
    "eves", "series", "species", "gallows", "bowels", "riches", "ashes",
];

pub fn stem(word: &str) -> String {
    let w = normalize(word);
    if let Some(&(_, base)) = IRREGULAR.iter().find(|(f, _)| *f == w) {
        return finish(base);
    }
    if KEEP.contains(&w.as_str()) || w.len() <= 3 {
        return w;
    }
    for suffix in ["edst", "ness", "eth", "est", "ing", "ed", "es", "s"] {
        if let Some(base) = w.strip_suffix(suffix) {
            if suffix == "s" && (base.ends_with('s') || base.ends_with('u') || base.ends_with('i')) {
                continue;
            }
            if base.len() >= 3 && base.chars().any(|c| "aeiouy".contains(c)) {
                return finish(base);
            }
        }
    }
    finish(&w)
}

fn finish(base: &str) -> String {
    let mut s: Vec<u8> = base.as_bytes().to_vec();
    let n = s.len();
    if n >= 4 && s[n - 1] == s[n - 2] && b"bdgmnprt".contains(&s[n - 1]) {
        s.pop();
    }
    if s.len() > 3 && s.last() == Some(&b'e') {
        s.pop();
    }
    if s.len() > 2 && s.last() == Some(&b'i') {
        *s.last_mut().unwrap() = b'y';
    }
    String::from_utf8(s).unwrap()
}

/// Function words and "that verse about..." filler. They boost a verse that
/// already matched but never pull one in on their own.
const STOP: &[&str] = &[
    "a", "an", "and", "as", "at", "be", "but", "by", "for", "from", "he", "her", "him", "his", "i", "if",
    "in", "into", "it", "its", "me", "my", "of", "on", "or", "our", "so", "that", "the", "their",
    "them", "then", "there", "they", "this", "to", "unto", "us", "we", "which", "who", "whom", "with",
    "you", "your", "shall", "have", "not", "upon", "what", "when", "where", "also", "do", "about",
    "something", "verse", "vers", "passage", "bible", "say", "talk", "mention", "quote", "scripture",
    "get", "will", "these", "those", "nor", "neither", "o", "oh", "how", "whose", "she", "one", "some",
    "thing", "very", "can", "could", "would", "should", "may", "might", "let", "go", "make",
];

pub fn is_stop(stem: &str) -> bool {
    STOP.contains(&stem)
}

/// Damerau-Levenshtein distance, giving up past `max`.
pub fn edit_distance(a: &[u8], b: &[u8], max: usize) -> Option<usize> {
    if a.len().abs_diff(b.len()) > max {
        return None;
    }
    let w = b.len() + 1;
    let mut prev2 = vec![0usize; w];
    let mut prev: Vec<usize> = (0..w).collect();
    let mut cur = vec![0usize; w];
    for i in 1..=a.len() {
        cur[0] = i;
        let mut row_min = i;
        for j in 1..=b.len() {
            let cost = (a[i - 1] != b[j - 1]) as usize;
            let mut v = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                v = v.min(prev2[j - 2] + 1);
            }
            cur[j] = v;
            row_min = row_min.min(v);
        }
        if row_min > max {
            return None;
        }
        std::mem::swap(&mut prev2, &mut prev);
        std::mem::swap(&mut prev, &mut cur);
    }
    (prev[b.len()] <= max).then_some(prev[b.len()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stems_agree_across_forms() {
        for group in [
            &["run", "runneth", "running", "ran"][..],
            &["weary", "wearied", "weariness"],
            &["love", "loved", "loveth", "lovest", "loving"],
            &["city", "cities"],
            &["church", "churches"],
            &["say", "saith", "said"],
        ] {
            let s = stem(group[0]);
            for w in group {
                assert_eq!(stem(w), s, "{w}");
            }
        }
        assert_ne!(stem("forest"), stem("for"));
        assert_eq!(normalize("Peter’s"), "peter");
        assert_eq!(normalize("Beth-el"), "bethel");
    }

    #[test]
    fn splits_words() {
        let s = "Peter’s brother, Beth-el; ‘go’";
        let w: Vec<&str> = words(s).iter().map(|&(a, b)| &s[a..b]).collect();
        assert_eq!(w, ["Peter’s", "brother", "Beth-el", "go"]);
    }
}
