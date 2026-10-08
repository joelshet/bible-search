//! The 66 books, their names, the abbreviations people type, and scope groups.

pub struct Book {
    pub code: &'static str,
    pub name: &'static str,
    /// What the map prints where the full name doesn't fit.
    pub short: &'static str,
    pub aliases: &'static [&'static str],
}

macro_rules! books {
    ($($code:literal $name:literal $short:literal [$($a:literal)*]),* $(,)?) => {
        pub const BOOKS: &[Book] = &[$(Book { code: $code, name: $name, short: $short, aliases: &[$($a),*] }),*];
    };
}

books! {
    "GEN" "Genesis" "Gen" ["gen" "ge" "gn"], "EXO" "Exodus" "Exod" ["exo" "ex" "exod"],
    "LEV" "Leviticus" "Lev" ["lev" "le" "lv"], "NUM" "Numbers" "Num" ["num" "nu" "nm" "nb"],
    "DEU" "Deuteronomy" "Deut" ["deut" "dt" "de" "deu"], "JOS" "Joshua" "Josh" ["josh" "jos" "jsh"],
    "JDG" "Judges" "Judg" ["judg" "jdg" "jg" "jdgs"], "RUT" "Ruth" "Ruth" ["rth" "ru" "rut"],
    "1SA" "1 Samuel" "1 Sam" ["1sam" "1sa" "1sm" "1s"], "2SA" "2 Samuel" "2 Sam" ["2sam" "2sa" "2sm" "2s"],
    "1KI" "1 Kings" "1 Kgs" ["1kgs" "1ki" "1kin" "1k"], "2KI" "2 Kings" "2 Kgs" ["2kgs" "2ki" "2kin" "2k"],
    "1CH" "1 Chronicles" "1 Chr" ["1chr" "1ch" "1chron"], "2CH" "2 Chronicles" "2 Chr" ["2chr" "2ch" "2chron"],
    "EZR" "Ezra" "Ezra" ["ezr" "ez"], "NEH" "Nehemiah" "Neh" ["neh" "ne"], "EST" "Esther" "Esth" ["esth" "est" "es"],
    "JOB" "Job" "Job" ["jb"], "PSA" "Psalms" "Ps" ["ps" "psa" "psalm" "pslm" "psm" "pss"],
    "PRO" "Proverbs" "Prov" ["prov" "pro" "prv" "pr"], "ECC" "Ecclesiastes" "Eccl" ["eccl" "ecc" "ec" "qoh"],
    "SNG" "Song of Solomon" "Song" ["song" "sos" "ss" "sng" "canticles" "songofsongs"],
    "ISA" "Isaiah" "Isa" ["isa" "is"], "JER" "Jeremiah" "Jer" ["jer" "je" "jr"], "LAM" "Lamentations" "Lam" ["lam" "la"],
    "EZK" "Ezekiel" "Ezek" ["ezek" "eze" "ezk"], "DAN" "Daniel" "Dan" ["dan" "da" "dn"], "HOS" "Hosea" "Hos" ["hos" "ho"],
    "JOL" "Joel" "Joel" ["jl" "joe" "jol"], "AMO" "Amos" "Amos" ["amo" "am"], "OBA" "Obadiah" "Obad" ["obad" "ob" "oba"],
    "JON" "Jonah" "Jonah" ["jon" "jnh"], "MIC" "Micah" "Mic" ["mic" "mc"], "NAM" "Nahum" "Nah" ["nah" "na" "nam"],
    "HAB" "Habakkuk" "Hab" ["hab" "hb"], "ZEP" "Zephaniah" "Zeph" ["zeph" "zep" "zp"], "HAG" "Haggai" "Hag" ["hag" "hg"],
    "ZEC" "Zechariah" "Zech" ["zech" "zec" "zc"], "MAL" "Malachi" "Mal" ["mal" "ml"],
    "MAT" "Matthew" "Matt" ["matt" "mt" "mat"], "MRK" "Mark" "Mark" ["mrk" "mk" "mr"], "LUK" "Luke" "Luke" ["luk" "lk"],
    "JHN" "John" "John" ["jn" "jhn" "joh"], "ACT" "Acts" "Acts" ["act" "ac"], "ROM" "Romans" "Rom" ["rom" "ro" "rm"],
    "1CO" "1 Corinthians" "1 Cor" ["1cor" "1co"], "2CO" "2 Corinthians" "2 Cor" ["2cor" "2co"],
    "GAL" "Galatians" "Gal" ["gal" "ga"], "EPH" "Ephesians" "Eph" ["eph" "ephes"],
    "PHP" "Philippians" "Phil" ["phil" "php" "pp"], "COL" "Colossians" "Col" ["col"],
    "1TH" "1 Thessalonians" "1 Thess" ["1thess" "1th" "1thes"], "2TH" "2 Thessalonians" "2 Thess" ["2thess" "2th" "2thes"],
    "1TI" "1 Timothy" "1 Tim" ["1tim" "1ti"], "2TI" "2 Timothy" "2 Tim" ["2tim" "2ti"], "TIT" "Titus" "Titus" ["tit" "ti"],
    "PHM" "Philemon" "Phlm" ["philem" "phm" "pm"], "HEB" "Hebrews" "Heb" ["heb"], "JAS" "James" "Jas" ["jas" "jm" "jam"],
    "1PE" "1 Peter" "1 Pet" ["1pet" "1pe" "1pt" "1p"], "2PE" "2 Peter" "2 Pet" ["2pet" "2pe" "2pt" "2p"],
    "1JN" "1 John" "1 John" ["1jn" "1jo" "1jhn"], "2JN" "2 John" "2 John" ["2jn" "2jo" "2jhn"],
    "3JN" "3 John" "3 John" ["3jn" "3jo" "3jhn"], "JUD" "Jude" "Jude" ["jud" "jd"],
    "REV" "Revelation" "Rev" ["rev" "re" "rv" "revelations"],
}

pub fn by_code(code: &str) -> Option<usize> {
    BOOKS.iter().position(|b| b.code == code)
}

fn squash(s: &str) -> String {
    let s = s.to_lowercase();
    let mut t: String = s.chars().filter(|c| c.is_alphanumeric()).collect();
    for (roman, n) in [("iii", "3"), ("ii", "2"), ("i", "1"), ("first", "1"), ("second", "2"), ("third", "3")] {
        let spaced = format!("{roman} ");
        if s.starts_with(&spaced) {
            t = format!("{n}{}", &t[roman.len()..]);
            break;
        }
    }
    t
}

/// Resolve what someone typed ("1 cor", "jn", "Song of Solomon", "ii kings") to a book.
pub fn find(typed: &str) -> Option<usize> {
    let t = squash(typed);
    if t.is_empty() {
        return None;
    }
    if let Some(i) = BOOKS.iter().position(|b| squash(b.name) == t || b.aliases.contains(&t.as_str())) {
        return Some(i);
    }
    if t.len() < 3 {
        return None;
    }
    let hits: Vec<usize> = (0..BOOKS.len()).filter(|&i| squash(BOOKS[i].name).starts_with(&t)).collect();
    (hits.len() == 1).then(|| hits[0])
}

/// Book ranges for `in:` scopes.
pub fn group(name: &str) -> Option<Vec<usize>> {
    let range = |a: &str, b: &str| Some((by_code(a)?..=by_code(b)?).collect());
    match name {
        "ot" | "old" => range("GEN", "MAL"),
        "nt" | "new" => range("MAT", "REV"),
        "law" | "torah" | "pentateuch" => range("GEN", "DEU"),
        "history" => range("JOS", "EST"),
        "poetry" | "wisdom" => range("JOB", "SNG"),
        "prophets" => range("ISA", "MAL"),
        "major" => range("ISA", "DAN"),
        "minor" => range("HOS", "MAL"),
        "gospels" | "gospel" => range("MAT", "JHN"),
        "epistles" | "letters" => range("ROM", "JUD"),
        "paul" | "pauline" => range("ROM", "PHM"),
        "general" => range("HEB", "JUD"),
        _ => find(name).map(|b| vec![b]),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reference {
    pub book: usize,
    pub chapter: Option<u32>,
    pub verses: Option<(u32, u32)>,
}

/// Parse "jn 3:16", "ps 23", "1 cor 13:4-7", "gen1.1". A bare book name is not a reference.
pub fn parse_reference(q: &str) -> Option<Reference> {
    let q = q.trim();
    let end = q
        .char_indices()
        .rev()
        .take_while(|&(_, c)| c.is_ascii_digit() || " :.-–".contains(c))
        .last()
        .map_or(q.len(), |(i, _)| i);
    // "1 john" leaves "1 " as the book prefix, so only digits after a letter count
    let (book_part, nums) = (q[..end].trim(), q[end..].trim());
    if nums.is_empty() || !book_part.chars().any(|c| c.is_alphabetic()) {
        return None;
    }
    let book = find(book_part)?;
    let mut parts = nums.split(|c| c == ':' || c == '.' || c == ' ').filter(|s| !s.is_empty());
    let chapter: u32 = parts.next()?.parse().ok()?;
    let verses = match parts.next() {
        None => None,
        Some(v) => {
            let mut r = v.split(|c| c == '-' || c == '–');
            let a: u32 = r.next()?.parse().ok()?;
            let b: u32 = r.next().filter(|s| !s.is_empty()).map_or(Some(a), |s| s.parse().ok())?;
            Some((a, b.max(a)))
        }
    };
    Some(Reference { book, chapter: Some(chapter), verses })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_books() {
        let code = |s| find(s).map(|i| BOOKS[i].code);
        assert_eq!(code("jn"), Some("JHN"));
        assert_eq!(code("1 cor"), Some("1CO"));
        assert_eq!(code("I Corinthians"), Some("1CO"));
        assert_eq!(code("phil"), Some("PHP"));
        assert_eq!(code("Song of Solomon"), Some("SNG"));
        assert_eq!(code("rev"), Some("REV"));
        assert_eq!(code("jo"), None);
        assert_eq!(code("1 john"), Some("1JN"));
    }

    #[test]
    fn parses_references() {
        let r = parse_reference("jn 3:16").unwrap();
        assert_eq!((BOOKS[r.book].code, r.chapter, r.verses), ("JHN", Some(3), Some((16, 16))));
        let r = parse_reference("1 cor 13:4-7").unwrap();
        assert_eq!((BOOKS[r.book].code, r.chapter, r.verses), ("1CO", Some(13), Some((4, 7))));
        let r = parse_reference("ps23").unwrap();
        assert_eq!((BOOKS[r.book].code, r.chapter, r.verses), ("PSA", Some(23), None));
        assert_eq!(parse_reference("1 john"), None);
        assert_eq!(parse_reference("love"), None);
    }
}
