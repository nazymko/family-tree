#!/usr/bin/env python3
"""Writes the invented family for the Map screenshots: <dir>/maps.ged and <dir>/ZZ.zip (a GeoNames-style country file).

Every person and village here is made up. Some places carry coordinates, some do not (those are for the helper).

    python -B scripts/make-maps-fixture.py <dir>
"""
import os
import random
import sys
import zipfile

# name -> (lat, lon) or None (no coordinates yet)
PLACES = {
    "Berezivka": (50.52, 30.28),
    "Dubky": (50.31, 31.05),
    "Hlukhiv": (51.68, 33.91),
    "Kyiv": (50.45, 30.52),
    "Myrne": (49.84, 24.03),
    "Lisne": (50.09, 28.66),
    "Zelenyi Hai": None,
    "Mala Dubka": None,
    "Sosnivka": None,
}
REGION = "Zelena oblast, Ukraine"
GIVEN_M = ["Ivan", "Petro", "Mykola", "Stepan", "Andrii", "Vasyl", "Oleksii", "Hryhorii"]
GIVEN_F = ["Maria", "Olena", "Kateryna", "Hanna", "Oksana", "Halyna", "Yaroslava", "Sofiia"]
SURNAMES = ["Koval", "Shevchuk", "Melnyk", "Bondar", "Tkachenko", "Lysenko", "Moroz", "Kravets"]


def place_lines(name, lvl):
    out = ["%d PLAC %s, %s" % (lvl, name, REGION)]
    c = PLACES[name]
    if c:
        out += ["%d MAP" % (lvl + 1), "%d LATI N%.4f" % (lvl + 2, c[0]), "%d LONG E%.4f" % (lvl + 2, c[1])]
    return out


def main(folder):
    os.makedirs(folder, exist_ok=True)
    rnd = random.Random(7)
    names = list(PLACES)
    people = []  # dicts

    def make(gen, idx, sex, surname):
        given = rnd.choice(GIVEN_M if sex == "M" else GIVEN_F)
        year = 1905 - gen * 28 + rnd.randrange(-3, 4)
        home = names[(idx * 3 + gen) % len(names)]
        moved = names[(idx * 5 + gen + 2) % len(names)]
        died = names[(idx + gen * 2 + 1) % len(names)]
        people.append(
            dict(n=len(people) + 1, sex=sex, given=given, surname=surname, birth=year, death=year + 55 + rnd.randrange(25),
                 home=home, moved=moved, died=died, gen=gen, idx=idx, famc=None, fams=None)
        )
        return people[-1]

    # Ivan Petrovych Koval and his ancestors, five generations: (gen, idx) with father 2*idx and mother 2*idx+1 above.
    root = make(0, 0, "M", "Koval")
    root["given"] = "Ivan"
    layer = [root]
    fams = []
    for gen in range(1, 5):
        nxt = []
        for k, child in enumerate(layer):
            father = make(gen, 2 * k, "M", child["surname"] if child["sex"] == "M" else SURNAMES[(gen + k) % len(SURNAMES)])
            mother = make(gen, 2 * k + 1, "F", SURNAMES[(gen * 2 + k + 3) % len(SURNAMES)])
            f = dict(n=len(fams) + 1, h=father, w=mother, kids=[child], year=child["birth"] - 1, home=father["home"])
            fams.append(f)
            father["fams"] = f["n"]
            mother["fams"] = f["n"]
            child["famc"] = f["n"]
            nxt += [father, mother]
        layer = nxt

    out = ["0 HEAD", "1 SOUR MAPS-FIXTURE", "1 GEDC", "2 VERS 5.5.1", "2 FORM LINEAGE-LINKED", "1 CHAR UTF-8"]
    for p in people:
        out += ["0 @I%d@ INDI" % p["n"], "1 NAME %s /%s/" % (p["given"], p["surname"]), "1 SEX " + p["sex"]]
        if p["n"] == 1:
            out[-2] = "1 NAME Ivan Petrovych /Koval/"
        out += ["1 BIRT", "2 DATE %d" % p["birth"]] + place_lines(p["home"], 2)
        out += ["1 RESI", "2 DATE %d" % (p["birth"] + 24)] + place_lines(p["moved"], 2)
        out += ["1 DEAT", "2 DATE %d" % p["death"]] + place_lines(p["died"], 2)
        if p["famc"]:
            out.append("1 FAMC @F%d@" % p["famc"])
        if p["fams"]:
            out.append("1 FAMS @F%d@" % p["fams"])
    for f in fams:
        out += ["0 @F%d@ FAM" % f["n"], "1 HUSB @I%d@" % f["h"]["n"], "1 WIFE @I%d@" % f["w"]["n"]]
        out += ["1 CHIL @I%d@" % k["n"] for k in f["kids"]]
        out += ["1 MARR", "2 DATE %d" % f["year"]] + place_lines(f["home"], 2)
    out.append("0 TRLR")
    with open(os.path.join(folder, "maps.ged"), "w", encoding="utf-8", newline="\n") as fh:
        fh.write("\n".join(out) + "\n")

    # GeoNames-style rows for an invented country "ZZ": the villages without coordinates and a few others.
    rows = [
        (1, "Zelenyi Hai", 50.11, 29.94, "01", 900),
        (2, "Mala Dubka", 50.27, 31.31, "01", 300),
        (3, "Sosnivka", 50.85, 30.71, "02", 1200),
        (4, "Sosnivka", 49.20, 25.10, "03", 400),
        (5, "Dubky", 50.31, 31.05, "01", 700),
        (6, "Berezivka", 50.52, 30.28, "01", 2500),
    ]
    text = "".join(
        "%d\t%s\t%s\t\t%.5f\t%.5f\tP\tPPL\tZZ\t\t%s\t\t\t\t%d\t\t\tEurope/Kyiv\t2020-01-01\n" % (i, n, n, la, lo, a1, pop)
        for i, n, la, lo, a1, pop in rows
    )
    with zipfile.ZipFile(os.path.join(folder, "ZZ.zip"), "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("ZZ.txt", text)
    print("wrote", folder, len(people), "people")


if __name__ == "__main__":
    main(sys.argv[1])
