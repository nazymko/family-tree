#!/usr/bin/env python3
"""Writes a synthetic GEDCOM 5.5.1 file (UTF-8) for the end-to-end and performance tests.

    python scripts/make-synthetic-ged.py OUT.ged [--persons 50000] [--seed 7]

Properties (all deterministic for a seed):
  * Latin, Cyrillic, Hebrew and CJK names (a lineage keeps the script of its founder; marrying-in spouses are Latin).
  * about 6% of the people carry no dated event at all.
  * shared events: some sibling pairs both list one census (same _EVID), every couple shares its MARR.
  * about 3% of the people carry an OBJE media reference (the files do not exist; they show up in the import report).
  * a "spine" line: surname Aaberg sorts first, and the home person (generation 3, with ancestors and descendants) is named in
    the sidecar file OUT.ged.meta.json together with counts and surnames that only definitely-living people carry.
"""

import argparse
import json
import random
import sys
import uuid
from collections import deque

TODAY_YEAR = 2026

LATIN_GIVEN_M = ["Karl", "Johann", "Friedrich", "Heinrich", "Wilhelm", "Ludwig", "August", "Georg", "Anton", "Emil", "Hans", "Otto"]
LATIN_GIVEN_F = ["Anna", "Maria", "Elisabeth", "Margarethe", "Sophie", "Charlotte", "Katharina", "Louise", "Clara", "Frieda", "Greta", "Ida"]
LATIN_SURNAMES = ["Mueller", "Schmidt", "Schneider", "Fischer", "Weber", "Meyer", "Wagner", "Becker", "Hoffmann", "Schulz", "Koch", "Richter"]
CYR_GIVEN_M = ["Иван", "Пётр", "Алексей", "Николай", "Дмитрий", "Сергей", "Михаил", "Фёдор"]
CYR_GIVEN_F = ["Анна", "Мария", "Елена", "Наталья", "Ольга", "Татьяна", "Софья", "Вера"]
CYR_SURNAMES = ["Иванов", "Петров", "Смирнов", "Кузнецов", "Попов", "Соколов", "Лебедев", "Козлов"]
HEB_GIVEN_M = ["דוד", "משה", "יוסף", "אברהם", "יעקב", "שמואל", "אהרן", "נתן"]
HEB_GIVEN_F = ["שרה", "רחל", "לאה", "רבקה", "מרים", "אסתר", "חנה", "דבורה"]
HEB_SURNAMES = ["כהן", "לוי", "מזרחי", "פרץ", "ביטון", "אברמוב", "גולדברג", "שפירא"]
CJK_GIVEN_M = ["太郎", "一郎", "健", "誠", "大輔", "伟", "强", "明"]
CJK_GIVEN_F = ["花子", "美咲", "陽子", "愛", "芳", "丽", "娜", "静"]
CJK_SURNAMES = ["山田", "田中", "佐藤", "鈴木", "王", "李", "张", "陈"]
SYLLABLES = ["bar", "den", "fel", "gor", "hal", "jun", "kel", "lom", "mor", "nel", "ost", "pra", "quin", "rav", "sol", "tur", "ulm", "vek", "wen", "zar"]
SCRIPTS = ["latin"] * 11 + ["cyr"] * 3 + ["heb"] * 2 + ["cjk"] * 3
REGIONS = ["Brandenburg", "Bavaria", "Saxony", "Silesia", "Galicia", "Moscow Governorate", "Kyoto Prefecture", "Jerusalem District"]

NS = uuid.UUID("12345678-1234-5678-1234-567812345678")


def pools(script):
    return {
        "latin": (LATIN_GIVEN_M, LATIN_GIVEN_F, LATIN_SURNAMES),
        "cyr": (CYR_GIVEN_M, CYR_GIVEN_F, CYR_SURNAMES),
        "heb": (HEB_GIVEN_M, HEB_GIVEN_F, HEB_SURNAMES),
        "cjk": (CJK_GIVEN_M, CJK_GIVEN_F, CJK_SURNAMES),
    }[script]


class Person:
    __slots__ = ("n", "sex", "given", "surname", "script", "birth", "death", "undated", "famc", "fams", "place", "media",
                 "spine", "census", "cite")

    def __init__(self, n):
        self.n = n
        self.famc = None
        self.fams = []
        self.census = None
        self.media = None
        self.cite = None
        self.spine = False


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--persons", type=int, default=50000)
    ap.add_argument("--seed", type=int, default=7)
    args = ap.parse_args()
    rnd = random.Random(args.seed)
    target = max(args.persons, 40)

    persons = []
    fams = []  # dicts: n, h, w, kids, year, place

    def new_person(sex, script, surname, birth):
        p = Person(len(persons) + 1)
        p.sex = sex
        p.script = script
        gm, gf, _ = pools(script)
        p.given = rnd.choice(gm if sex == "M" else gf)
        p.surname = surname
        p.birth = birth
        p.undated = False
        p.death = None
        p.place = rnd.randrange(120)
        persons.append(p)
        return p

    def outsider(sex, birth):
        sur = "".join(rnd.choice(SYLLABLES) for _ in range(3)).capitalize() + str(rnd.randrange(40))
        return new_person(sex, "latin", sur, birth)

    queue = deque()  # couples waiting for children: fam dicts

    def marry(h, w, year):
        f = {"n": len(fams) + 1, "h": h, "w": w, "kids": [], "year": year, "place": rnd.randrange(120)}
        fams.append(f)
        h.fams.append(f)
        w.fams.append(f)
        queue.append(f)
        return f

    founders = max(2, target // 700)
    for i in range(founders):
        # The first founders cycle through every script so that each one is present even in a small file.
        script = ["latin", "cyr", "heb", "cjk"][i % 4] if i < 8 else rnd.choice(SCRIPTS)
        _, _, surs = pools(script)
        by = 1690 + rnd.randrange(25)
        sur = "Aldous" if i == 0 else rnd.choice(surs)
        script = "latin" if i == 0 else script
        h = new_person("M", script, sur, by)
        w = new_person("F", script, sur if script == "cjk" else rnd.choice(pools(script)[2]), by + rnd.randrange(-3, 6))
        h.spine = i == 0
        marry(h, w, by + 22 + rnd.randrange(8))

    home = None
    while len(persons) < target:
        if not queue:  # ran out of descendants: another founding couple
            by = 1700 + rnd.randrange(60)
            script = rnd.choice(SCRIPTS)
            sur = rnd.choice(pools(script)[2])
            h = new_person("M", script, sur, by)
            w = new_person("F", script, rnd.choice(pools(script)[2]), by + 1)
            marry(h, w, by + 24)
            continue
        f = queue.popleft()
        h, w = f["h"], f["w"]
        spine_couple = h.spine
        nkids = 4 if spine_couple else rnd.choices([0, 1, 2, 3, 4], [1, 2, 4, 3, 1])[0]
        by = f["year"] + 1
        for k in range(nkids):
            if len(persons) >= target:
                break
            yr = by + k * 2 + rnd.randrange(2)
            if yr > TODAY_YEAR - 1:
                break
            sex = "M" if (spine_couple and k == 0) else rnd.choice("MF")
            sur = h.surname
            c = new_person(sex, h.script, sur, yr)
            c.famc = f
            f["kids"].append(c)
            if spine_couple and k == 0:
                c.spine = True
            # a spine child marries in; others marry with 80%
            if (c.spine or rnd.random() < 0.8) and yr + 24 < TODAY_YEAR:
                oc = outsider("F" if sex == "M" else "M", yr + rnd.randrange(-4, 5))
                if sex == "M":
                    marry(c, oc, yr + 22 + rnd.randrange(10))
                else:
                    marry(oc, c, yr + 22 + rnd.randrange(10))
            if len(persons) >= target:
                break

    # Spine bookkeeping: walk first sons from the first couple; the third son-of-son (generation 3) is the home person.
    chain = [persons[0]]
    while True:
        cur = chain[-1]
        nxt = next((c for fam in cur.fams for c in fam["kids"] if c.spine), None)
        if nxt is None:
            break
        chain.append(nxt)
    if len(chain) < 5:
        sys.exit("spine too short; use more persons")
    home = chain[3]
    # From the home person on, the male line carries the sorts-first surname.
    def rename_line(p):
        p.surname = "Aaberg"
        for fam in p.fams:
            for c in fam["kids"]:
                if c.sex == "M":
                    rename_line(c)
                else:
                    c.surname = "Aaberg"
    sys.setrecursionlimit(10000)
    rename_line(home)
    home.given = "Bartholomaeus"

    # Dates, deaths, undated, media, shared events.
    for p in persons:
        r = rnd.random()
        p.undated = (r < 0.06) and not p.spine
        if not p.undated and p.birth < 1930:
            age = 45 + rnd.randrange(50)
            if p.birth + age < TODAY_YEAR:
                p.death = p.birth + age
        if rnd.random() < 0.03:
            p.media = p.n
        if rnd.random() < 0.10:
            p.cite = 1 + rnd.randrange(20)
    for f in fams:
        kids = f["kids"]
        if len(kids) >= 2 and rnd.random() < 0.25:
            ev = str(uuid.uuid5(NS, "census-%d" % f["n"]))
            for k in kids[:2]:
                if not k.undated:
                    k.census = (ev, f["year"] + 10 + rnd.randrange(5))

    def fmt_year(y):
        r = rnd.random()
        if r < 0.80:
            return str(y)
        if r < 0.88:
            return "ABT %d" % y
        if r < 0.94:
            return "BEF %d" % y
        if r < 0.97:
            return "BET %d AND %d" % (y - 2, y + 2)
        return "%d %s %d" % (1 + rnd.randrange(28), rnd.choice(["JAN", "MAR", "JUN", "SEP", "DEC"]), y)

    def place(i):
        r = i % 8
        if i % 6 == 0:
            return "Город%d, %s, Россия" % (i, "Московская губерния")
        return "Town%d, %s, Germany" % (i, REGIONS[r])

    def name_line(p):
        return "%s /%s/" % (p.given, p.surname)

    out = []
    w = out.append
    w("0 HEAD")
    w("1 SOUR SYNTHETIC")
    w("2 VERS 1.0")
    w("2 NAME Gene Tree synthetic generator")
    w("1 GEDC")
    w("2 VERS 5.5.1")
    w("2 FORM LINEAGE-LINKED")
    w("1 CHAR UTF-8")
    for s in range(1, 21):
        w("0 @S%d@ SOUR" % s)
        w("1 TITL Synthetic source %d" % s)
    for p in persons:
        w("0 @I%d@ INDI" % p.n)
        w("1 NAME " + name_line(p))
        w("2 GIVN " + p.given)
        w("2 SURN " + p.surname)
        w("1 SEX " + p.sex)
        if not p.undated:
            w("1 BIRT")
            w("2 DATE " + fmt_year(p.birth))
            w("2 PLAC " + place(p.place))
            if p.cite:
                w("2 SOUR @S%d@" % p.cite)
                w("3 PAGE p. %d" % (p.n % 300))
            if p.death:
                w("1 DEAT")
                w("2 DATE " + fmt_year(p.death))
                w("2 PLAC " + place((p.place * 7 + 3) % 120))
        elif p.n % 3 == 0:
            w("1 BIRT")
            w("2 PLAC " + place(p.place))
        if p.census:
            w("1 CENS")
            w("2 DATE %d" % p.census[1])
            w("2 _EVID " + p.census[0])
        if p.media:
            w("1 OBJE")
            w("2 FILE media/photo-%d.jpg" % p.media)
            w("3 FORM jpg")
            w("2 TITL Portrait %d" % p.media)
        if p.famc:
            w("1 FAMC @F%d@" % p.famc["n"])
        for fam in p.fams:
            w("1 FAMS @F%d@" % fam["n"])
    for f in fams:
        w("0 @F%d@ FAM" % f["n"])
        w("1 HUSB @I%d@" % f["h"].n)
        w("1 WIFE @I%d@" % f["w"].n)
        if not (f["h"].undated or f["w"].undated):
            w("1 MARR")
            w("2 DATE %d" % f["year"])
            w("2 PLAC " + place(f["place"]))
        for c in f["kids"]:
            w("1 CHIL @I%d@" % c.n)
    w("0 TRLR")
    with open(args.out, "w", encoding="utf-8", newline="\n") as fh:
        fh.write("\n".join(out) + "\n")

    # Sidecar: what the tests assert against.
    definite_living = [p for p in persons if not p.undated and p.birth >= 1950 and p.death is None]
    living_ids = {p.n for p in definite_living}
    other_surnames = {p.surname for p in persons if p.n not in living_ids}
    unique_living = sorted({p.surname for p in definite_living} - other_surnames)
    sample = {}
    for script in ("latin", "cyr", "heb", "cjk"):
        hit = next((p for p in persons if p.script == script and p.spine is False and p.surname not in ("Aaberg", "Aldous")), None)
        if hit:
            sample[script] = {"given": hit.given, "surname": hit.surname}
    meta = {
        "persons": len(persons),
        "families": len(fams),
        "undated": sum(1 for p in persons if p.undated),
        "withMedia": sum(1 for p in persons if p.media),
        "home": {"given": home.given, "surname": home.surname, "name": "%s %s" % (home.given, home.surname)},
        "uniqueLivingSurnames": unique_living[:200],
        "scripts": sample,
    }
    with open(args.out + ".meta.json", "w", encoding="utf-8", newline="\n") as fh:
        json.dump(meta, fh, ensure_ascii=False, indent=2)
    print("wrote %d people, %d families to %s" % (len(persons), len(fams), args.out))


if __name__ == "__main__":
    main()
