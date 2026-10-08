#!/usr/bin/env python3
"""Download the three public sources and flatten them into the TSVs in data/.

Run once; the outputs are committed, so building the index never needs the network.

  data/kjv.tsv       BOOK  chapter  verse  marked-up text (verse 0 = psalm title)
  data/strongs.tsv   id  lemma  translit  pron  derivation  definition  kjv_usage
  data/wordnet.tsv   word  synonym:weight ... (weight 0-100, strongest first)

Text markup in kjv.tsv: {words|H7225} Strong's-tagged span, [words] translator
italics, <words> words of Jesus.
"""

import io
import json
import re
import sys
import tarfile
import unicodedata
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RAW = ROOT / "data" / "raw"
OUT = ROOT / "data"

KJV_URL = "https://ebible.org/Scriptures/eng-kjv_usfm.zip"
STRONGS_URL = "https://raw.githubusercontent.com/openscriptures/strongs/master/{0}/strongs-{0}-dictionary.js"
WORDNET_URL = "https://wordnetcode.princeton.edu/3.0/WNdb-3.0.tar.gz"

BOOKS = (
    "GEN EXO LEV NUM DEU JOS JDG RUT 1SA 2SA 1KI 2KI 1CH 2CH EZR NEH EST JOB PSA PRO "
    "ECC SNG ISA JER LAM EZK DAN HOS JOL AMO OBA JON MIC NAM HAB ZEP HAG ZEC MAL "
    "MAT MRK LUK JHN ACT ROM 1CO 2CO GAL EPH PHP COL 1TH 2TH 1TI 2TI TIT PHM HEB "
    "JAS 1PE 2PE 1JN 2JN 3JN JUD REV"
).split()


def fetch(url, name):
    path = RAW / name
    if not path.exists():
        print(f"downloading {url}", file=sys.stderr)
        RAW.mkdir(parents=True, exist_ok=True)
        with urllib.request.urlopen(url) as r:
            path.write_bytes(r.read())
    return path.read_bytes()


def strong_code(code):
    return code[0] + str(int(code[1:]))


def usfm_inline(s):
    s = re.sub(r"\\f .*?\\f\*", "", s)
    s = re.sub(r"\\\+?w ([^|\\]*)\|strong=\"([^\"]*)\"\\\+?w\*",
               lambda m: "{%s|%s}" % (m[1].strip(), ",".join(strong_code(c) for c in m[2].split())), s)
    s = re.sub(r"\\\+?add\*", "]", s)
    s = re.sub(r"\\\+?add ?", "[", s)
    s = re.sub(r"\\wj\*", ">", s)
    s = re.sub(r"\\wj ?", "<", s)
    s = re.sub(r"\\\+?nd\*?", "", s)
    s = re.sub(r"\\(q\d?|p|m|b)\b", " ", s)
    if "\\" in s:
        raise ValueError(f"unhandled USFM in: {s}")
    return s


def kjv():
    z = zipfile.ZipFile(io.BytesIO(fetch(KJV_URL, "eng-kjv_usfm.zip")))
    files = {n[3:6]: n for n in z.namelist() if n.endswith(".usfm") and n[3:6] in BOOKS}
    rows = []
    for book in BOOKS:
        chapter, verse, title = 0, None, None
        for line in z.read(files[book]).decode("utf-8").splitlines():
            line = line.strip()
            tag = line.split(" ", 1)[0]
            if tag == "\\c":
                chapter, verse = int(line.split()[1]), None
            elif tag == "\\d":
                rows.append([book, chapter, 0, usfm_inline(line[3:])])
            elif tag == "\\v":
                _, n, rest = (line + " ").split(" ", 2)
                verse = int(n)
                rows.append([book, chapter, verse, usfm_inline(rest)])
            elif tag in ("\\q1", "\\p", "\\b", "\\m", ""):
                if verse is not None and line[len(tag):].strip():
                    rows[-1][3] += " " + usfm_inline(line[len(tag):])
            # \id \h \toc \mt \s1 (Psalm 119 letters) \ms1 and intro lines carry no verse text
    with open(OUT / "kjv.tsv", "w", encoding="utf-8") as f:
        for book, c, v, text in rows:
            text = re.sub(r"\s+", " ", text).strip()
            text = re.sub(r" ([,.;:?!)>\]])", r"\1", text)
            text = re.sub(r"([<\[(]) ", r"\1", text)
            f.write(f"{book}\t{c}\t{v}\t{text}\n")
    print(f"kjv.tsv: {sum(1 for r in rows if r[2])} verses", file=sys.stderr)


def strongs():
    out = []
    for lang in ("hebrew", "greek"):
        js = fetch(STRONGS_URL.format(lang), f"strongs-{lang}.js").decode("utf-8")
        entries = json.loads(js[js.index("{"):js.rindex("}") + 1])
        for key, e in entries.items():
            fields = [e.get("lemma", ""), e.get("xlit") or e.get("translit", ""), e.get("pron", ""),
                      e.get("derivation", ""), e.get("strongs_def", ""), e.get("kjv_def", "")]
            out.append((key[0], int(key[1:]), [re.sub(r"\s+", " ", x).strip() for x in fields]))
    out.sort()
    with open(OUT / "strongs.tsv", "w", encoding="utf-8") as f:
        for lang, n, fields in out:
            f.write("\t".join([f"{lang}{n}"] + fields) + "\n")
    print(f"strongs.tsv: {len(out)} entries", file=sys.stderr)


def wordnet():
    tar = tarfile.open(fileobj=io.BytesIO(fetch(WORDNET_URL, "WNdb-3.0.tar.gz")))
    synsets = {}  # offset key -> (words, similar-to keys)
    index = {}    # word -> [synset keys], most common sense first
    for pos in ("noun", "verb", "adj", "adv"):
        for line in tar.extractfile(f"dict/data.{pos}").read().decode("latin-1").splitlines():
            if line.startswith(" "):
                continue
            parts = line.split()
            n = int(parts[3], 16)
            words = [parts[4 + 2 * i].lower().split("(")[0] for i in range(n)]
            p = 4 + 2 * n
            similar = [f"{parts[p + 2 + 4 * i]}{parts[p + 3 + 4 * i]}" for i in range(int(parts[p]))
                       if parts[p + 1 + 4 * i] == "&"]
            synsets[f"{parts[0]}{pos[0]}"] = (words, similar)
        for line in tar.extractfile(f"dict/index.{pos}").read().decode("latin-1").splitlines():
            if line.startswith(" "):
                continue
            parts = line.split()
            word, count = parts[0], int(parts[2])
            offsets = parts[-count:]
            index.setdefault(word, []).extend(f"{o}{pos[0]}" for o in offsets)
    pos_fix = {"a": "a", "s": "a"}
    with open(OUT / "wordnet.tsv", "w", encoding="utf-8") as f:
        for word, keys in sorted(index.items()):
            if not word.isalpha():
                continue
            # weight: same synset beats "similar to"; common senses beat rare ones
            weights = {}
            for rank, key in enumerate(keys):
                words, similar = synsets[key]
                fade = 1 / (1 + 0.5 * rank)
                for s in words:
                    weights[s] = max(weights.get(s, 0), fade)
                for sim in similar:
                    for s in synsets.get(sim[:-1] + pos_fix.get(sim[-1], sim[-1]), ([], []))[0]:
                        weights[s] = max(weights.get(s, 0), 0.6 * fade)
            weights.pop(word, None)
            keep = sorted(((w, s) for s, w in weights.items() if s.isalpha()), key=lambda x: (-x[0], x[1]))
            if keep:
                f.write(word + "\t" + " ".join(f"{s}:{round(w * 100)}" for w, s in keep) + "\n")
    print("wordnet.tsv written", file=sys.stderr)


if __name__ == "__main__":
    kjv()
    strongs()
    wordnet()
