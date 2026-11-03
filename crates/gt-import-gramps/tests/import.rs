//! Gramps XML import against a hand-written, synthetic fixture (no real data).

use flate2::Compression;
use flate2::write::GzEncoder;
use gt_core::date::{DateValue, GenDate};
use gt_core::model::*;
use gt_core::tree::Tree;
use gt_gedcom::{ImportOptions, ImportOutcome, MissingReason, Repair};
use gt_import_gramps::{GrampsError, import_gramps, is_gzip};
use std::io::Write;

const FIXTURE: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE database PUBLIC "-//GRAMPS//DTD GRAMPS XML 1.7.1//EN"
"http://gramps-project.org/xml/1.7.1/grampsxml.dtd">
<database xmlns="http://gramps-project.org/xml/1.7.1/">
  <header>
    <created date="2026-01-02" version="5.2.0"/>
    <researcher><resname>Test Researcher</resname></researcher>
    <mediapath>/somewhere/else</mediapath>
  </header>
  <tags>
    <tag handle="_tag1" name="ToDo" color="#ff0000" priority="1" change="1700000000"/>
  </tags>
  <events>
    <event handle="_e1" id="E0001" change="1700000000">
      <type>Birth</type>
      <dateval val="1850-05-12" type="about"/>
      <place hlink="_pl1"/>
      <description>born at home</description>
      <citationref hlink="_c1"/>
    </event>
    <event handle="_e2" id="E0002">
      <type>Death</type>
      <dateval val="1920-00-00" type="before" quality="estimated"/>
      <cause>influenza</cause>
      <attribute type="Age" value="70"/>
      <attribute type="Mood" value="calm"/>
    </event>
    <event handle="_e3" id="E0003">
      <type>Birth</type>
      <daterange start="1875-01-01" stop="1875-12-31"/>
      <place hlink="_pl1"/>
    </event>
    <event handle="_e4" id="E0004">
      <type>Baptism</type>
      <datespan from="1875-02-01" to="1875-03-01"/>
    </event>
    <event handle="_e5" id="E0005">
      <type>Residence</type>
      <dateval val="1700-03-01" cformat="Julian"/>
    </event>
    <event handle="_e6" id="E0006">
      <type>Marriage</type>
      <dateval val="1870-06-00" quality="calculated"/>
      <noteref hlink="_n1"/>
    </event>
    <event handle="_e7" id="E0007">
      <type>Pilgrimage</type>
      <datestr val="sometime in the spring"/>
    </event>
    <event handle="_e8" id="E0008">
      <type>Immigration</type>
      <dateval val="1900-01-01" cformat="Islamic"/>
    </event>
    <event handle="_e9" id="E0009">
      <type>Occupation</type>
      <dateval val="1880-00-00" quality="estimated"/>
      <description>farmer</description>
    </event>
  </events>
  <people>
    <person handle="_p1" id="I0001" change="1700000001">
      <gender>M</gender>
      <name type="Birth Name">
        <first>Jan</first>
        <call>Jan</call>
        <nick>Johnny</nick>
        <title>Dr</title>
        <suffix>Jr</suffix>
        <surname prim="1" prefix="van der">Berg</surname>
        <citationref hlink="_c1"/>
        <noteref hlink="_n1"/>
      </name>
      <name alt="1" type="Also Known As">
        <first>Johnny</first>
        <surname>Mountain</surname>
      </name>
      <eventref hlink="_e1" role="Primary"/>
      <eventref hlink="_e2" role="Primary"/>
      <eventref hlink="_e5" role="Primary"/>
      <eventref hlink="_nope" role="Primary"/>
      <objref hlink="_o1">
        <region corner1_x="10" corner1_y="20" corner2_x="60" corner2_y="80"/>
      </objref>
      <attribute type="Caste" value="none"/>
      <address><street>1 Main St</street></address>
      <parentin hlink="_f1"/>
      <tagref hlink="_tag1"/>
    </person>
    <person handle="_p2" id="I0002" priv="1">
      <gender>F</gender>
      <name type="Birth Name">
        <first>Maria</first>
        <surname>Jones</surname>
      </name>
      <name alt="1" type="Married Name">
        <first>Maria</first>
        <surname>Berg</surname>
      </name>
      <eventref hlink="_e9" role="Primary"/>
      <parentin hlink="_f1"/>
    </person>
    <person handle="_p3" id="I0003">
      <gender>M</gender>
      <name type="Birth Name">
        <first>Ivan</first>
        <surname derivation="Patronymic">Petrovich</surname>
        <surname prim="1">Smirnov</surname>
        <datestr val="about 1880"/>
      </name>
      <eventref hlink="_e3" role="Primary"/>
      <eventref hlink="_e4" role="Primary"/>
      <eventref hlink="_e6" role="Witness"/>
      <eventref hlink="_e7" role="Primary"/>
      <eventref hlink="_e8" role="Primary"/>
      <childof hlink="_f1"/>
      <personref hlink="_p1" rel="Godfather"/>
    </person>
    <person handle="_p4" id="I0004">
      <gender>U</gender>
      <name><first>Unnamed</first></name>
    </person>
  </people>
  <families>
    <family handle="_f1" id="F0001">
      <rel type="Married"/>
      <father hlink="_p1"/>
      <mother hlink="_p2"/>
      <eventref hlink="_e6" role="Family"/>
      <childref hlink="_p3" frel="Birth" mrel="Adopted">
        <citationref hlink="_c1"/>
      </childref>
      <childref hlink="_p4" frel="Stepchild" mrel="Sponsored"/>
      <attribute type="Class" value="gentry"/>
      <noteref hlink="_n2"/>
    </family>
    <family handle="_f2" id="F0002">
      <rel type="Civil Union"/>
      <father hlink="_p4"/>
    </family>
  </families>
  <citations>
    <citation handle="_c1" id="C0001" change="1700000002">
      <dateval val="1851-01-01" type="before"/>
      <page>p. 12</page>
      <confidence>3</confidence>
      <noteref hlink="_n1"/>
      <sourceref hlink="_s1"/>
      <srcattribute type="Film" value="42"/>
    </citation>
  </citations>
  <sources>
    <source handle="_s1" id="S0001">
      <stitle>Parish Register</stitle>
      <sauthor>The Vicar</sauthor>
      <spubinfo>Parish Press</spubinfo>
      <sabbrev>PR</sabbrev>
      <reporef hlink="_r1" callno="A-12" medium="Book"/>
    </source>
  </sources>
  <places>
    <placeobj handle="_pl1" id="P0001" type="City">
      <ptitle>Springfield, Illinois, USA</ptitle>
      <pname value="Springfield"/>
      <pname value="Springfeld" lang="de"/>
      <coord long="-89.65" lat="39.8"/>
      <placeref hlink="_pl2"/>
    </placeobj>
    <placeobj handle="_pl2" id="P0002" type="State">
      <pname value="Illinois"/>
      <placeref hlink="_pl3"/>
    </placeobj>
    <placeobj handle="_pl3" id="P0003" type="Country">
      <pname value="USA"/>
      <coord long="W98&#176;30'0&quot;" lat="N39&#176;30'0&quot;"/>
    </placeobj>
  </places>
  <objects>
    <object handle="_o1" id="O0001">
      <file src="photos\portrait.jpg" mime="image/jpeg" checksum="abc" description="Portrait"/>
      <dateval val="1900-00-00"/>
    </object>
    <object handle="_o2" id="O0002">
      <file src="missing/file.png" mime="image/png" description=""/>
    </object>
  </objects>
  <repositories>
    <repository handle="_r1" id="R0001">
      <rname>County Archive</rname>
      <type>Archive</type>
      <address><street>2 Oak Rd</street><city>Springfield</city><country>USA</country></address>
      <url href="https://example.org/archive" type="Web Home"/>
    </repository>
  </repositories>
  <notes>
    <note handle="_n1" id="N0001" type="General" format="0">
      <text>A shared note &amp; more.</text>
    </note>
    <note handle="_n2" id="N0002" type="Research">
      <text>Needs checking.</text>
      <style name="bold"><range start="0" end="5"/></style>
    </note>
  </notes>
  <bookmarks><bookmark target="person" hlink="_p1"/></bookmarks>
</database>
"##;

fn gz(s: &str) -> Vec<u8> {
    let mut e = GzEncoder::new(Vec::new(), Compression::default());
    e.write_all(s.as_bytes()).expect("write gz");
    e.finish().expect("finish gz")
}

fn run(s: &str) -> ImportOutcome {
    import_gramps(&gz(s), &ImportOptions::default()).expect("import")
}

fn xref_is(c: &Common, x: &str) -> bool {
    c.ext_ids
        .iter()
        .any(|e| e.kind == ExtIdKind::Xref && e.value == x)
}

fn person<'a>(t: &'a Tree, x: &str) -> &'a Person {
    t.persons
        .values()
        .find(|p| xref_is(&p.common, x))
        .expect("person")
}

fn event<'a>(t: &'a Tree, x: &str) -> &'a Event {
    t.events
        .values()
        .find(|e| xref_is(&e.common, x))
        .expect("event")
}

fn date_of(t: &Tree, x: &str) -> GenDate {
    event(t, x).date.clone().expect("date")
}

fn text(g: &GenDate) -> String {
    g.to_gedcom(gt_core::GedcomVersion::V551).0
}

#[test]
fn gzip_and_plain_give_the_same_tree() {
    let a = import_gramps(&gz(FIXTURE), &ImportOptions::default()).expect("gz");
    let b = import_gramps(FIXTURE.as_bytes(), &ImportOptions::default()).expect("plain");
    assert!(is_gzip(&gz(FIXTURE)) && !is_gzip(FIXTURE.as_bytes()));
    assert_eq!(a.tree, b.tree);
    // The GEDCOM version field stays empty; the XML format version is part of the producer.
    assert_eq!(a.report.version, None);
    assert_eq!(
        a.report.producer.as_deref(),
        Some("Gramps 5.2.0 (XML format 1.7.1)")
    );
    assert_eq!(a.report.counts.get("person"), Some(&4));
}

#[test]
fn ids_are_stable_across_imports() {
    let a = run(FIXTURE);
    let b = run(FIXTURE);
    assert_eq!(a.tree, b.tree);
}

#[test]
fn names_parts_and_sex() {
    let t = run(FIXTURE).tree;
    let p1 = person(&t, "I0001");
    assert_eq!(p1.sex, Sex::Male);
    assert_eq!(p1.names.len(), 2);
    let n = &p1.names[0];
    assert_eq!(n.kind, NameKind::Birth);
    assert!(n.assertion.preferred);
    assert_eq!(n.given.as_deref(), Some("Jan"));
    assert_eq!(n.call.as_deref(), Some("Jan"));
    assert_eq!(n.nickname.as_deref(), Some("Johnny"));
    assert_eq!(n.prefix.as_deref(), Some("Dr"));
    assert_eq!(n.suffix.as_deref(), Some("Jr"));
    assert_eq!(n.surname_prefix.as_deref(), Some("van der"));
    assert_eq!(n.surnames, vec!["Berg"]);
    assert_eq!(n.raw.as_deref(), Some("Jan /van der Berg/ Jr"));
    assert_eq!(n.citations.len(), 1);
    assert_eq!(n.notes.len(), 1);
    assert_eq!(p1.names[1].kind, NameKind::Aka);
    assert!(!p1.names[1].assertion.preferred);

    let p2 = person(&t, "I0002");
    assert_eq!(p2.sex, Sex::Female);
    assert!(p2.common.private);
    assert_eq!(p2.names[1].kind, NameKind::Married);

    let p3 = person(&t, "I0003");
    let n = &p3.names[0];
    assert_eq!(n.patronymic.as_deref(), Some("Petrovich"));
    assert_eq!(n.surnames, vec!["Smirnov"]);
    assert_eq!(n.raw.as_deref(), Some("Ivan Petrovich /Smirnov/"));
    assert!(n.date.is_some());

    assert_eq!(person(&t, "I0004").sex, Sex::Unknown);
    assert_eq!(person(&t, "I0004").names[0].kind, NameKind::Birth);
}

#[test]
fn date_semantics() {
    let t = run(FIXTURE).tree;
    // about
    let g = date_of(&t, "E0001");
    assert!(matches!(g.value, DateValue::About(_)));
    assert_eq!(text(&g), "ABT 12 MAY 1850");
    // before + estimated -> Before with the quality kept as a phrase; partial date (year only)
    let g = date_of(&t, "E0002");
    assert!(matches!(g.value, DateValue::Before(ref d) if d.month.is_none() && d.year == 1920));
    assert_eq!(g.phrase.as_deref(), Some("estimated"));
    // daterange = between
    let g = date_of(&t, "E0003");
    assert!(matches!(g.value, DateValue::Between(..)));
    assert_eq!(text(&g), "BET 1 JAN 1875 AND 31 DEC 1875");
    // datespan = from/to
    let g = date_of(&t, "E0004");
    assert!(matches!(g.value, DateValue::FromTo(..)));
    // Julian calendar kept
    let g = date_of(&t, "E0005");
    match g.value {
        DateValue::Exact(d) => {
            assert_eq!(d.calendar, gt_core::date::calendar::Calendar::Julian);
            assert_eq!((d.day, d.month, d.year), (Some(1), Some(3), 1700));
        }
        other => panic!("expected exact, got {other:?}"),
    }
    // calculated, month precision
    let g = date_of(&t, "E0006");
    assert!(
        matches!(g.value, DateValue::Calculated(ref d) if d.month == Some(6) && d.day.is_none())
    );
    // estimated
    assert!(matches!(
        date_of(&t, "E0009").value,
        DateValue::Estimated(_)
    ));
}

#[test]
fn unparsed_and_unsupported_dates_are_reported() {
    let out = run(FIXTURE);
    // datestr that is no date: kept as unparsed text and listed
    assert!(matches!(
        date_of(&out.tree, "E0007").value,
        DateValue::Unparsed
    ));
    assert!(
        out.report
            .unparsed_dates
            .iter()
            .any(|(_, d)| d == "sometime in the spring")
    );
    // Islamic calendar: phrase only, counted
    let g = date_of(&out.tree, "E0008");
    assert!(matches!(g.value, DateValue::PhraseOnly));
    assert!(g.phrase.as_deref().is_some_and(|p| p.contains("Islamic")));
    assert_eq!(
        out.report.unmapped_tags.get("date/calendar islamic"),
        Some(&1)
    );
    // datestr that parses is a real date
    let t = &out.tree;
    let n = &person(t, "I0003").names[0];
    assert!(matches!(
        n.date.as_ref().map(|d| &d.value),
        Some(DateValue::About(_))
    ));
}

#[test]
fn events_roles_places_and_attributes() {
    let t = run(FIXTURE).tree;
    let birth = event(&t, "E0001");
    assert_eq!(birth.kind, EventKind::Birth);
    assert_eq!(birth.description.as_deref(), Some("born at home"));
    assert_eq!(birth.common.citations.len(), 1);
    assert_eq!(birth.common.changed, Some(1_700_000_000_000));
    let p1 = person(&t, "I0001").common.id;
    assert_eq!(
        birth.participants,
        vec![Participant {
            target: ParticipantRef::Person(p1),
            role: Role::Principal
        }]
    );
    let death = event(&t, "E0002");
    assert_eq!(death.cause.as_deref(), Some("influenza"));
    assert_eq!(death.age.as_deref(), Some("70"));
    assert!(death.common.unknown.iter().any(|r| {
        r.tag == "attribute"
            && r.children
                .iter()
                .any(|c| c.value.as_deref() == Some("Mood"))
    }));
    assert_eq!(
        event(&t, "E0007").kind,
        EventKind::Custom("Pilgrimage".to_string())
    );

    // The shared marriage: family principal plus a person witness, principal first.
    let m = event(&t, "E0006");
    assert_eq!(m.kind, EventKind::Marriage);
    assert_eq!(m.participants.len(), 2);
    assert!(matches!(
        m.participants[0],
        Participant {
            target: ParticipantRef::Family(_),
            role: Role::Principal
        }
    ));
    assert_eq!(m.participants[1].role, Role::Witness);
    assert!(
        t.events_of(person(&t, "I0003").common.id)
            .any(|(e, r)| e.kind == EventKind::Marriage && *r == Role::Witness)
    );

    // Place hierarchy, names, coordinates, chain text.
    let place = t
        .places
        .get(&birth.place.expect("place"))
        .expect("place rec");
    assert_eq!(place.names[0].name, "Springfield");
    assert_eq!(place.names[1].lang.as_deref(), Some("de"));
    assert_eq!(place.kind.as_deref(), Some("City"));
    assert_eq!((place.lat, place.lon), (Some(39.8), Some(-89.65)));
    assert_eq!(place.raw.as_deref(), Some("Springfield, Illinois, USA"));
    let state = t.places.get(&place.parent.expect("state")).expect("state");
    assert_eq!(state.names[0].name, "Illinois");
    let country = t
        .places
        .get(&state.parent.expect("country"))
        .expect("country");
    assert_eq!(country.parent, None);
    // degree/minute/second coordinates
    assert_eq!((country.lat, country.lon), (Some(39.5), Some(-98.5)));
}

#[test]
fn families_children_and_preferred_links() {
    let t = run(FIXTURE).tree;
    let fam = t
        .families
        .values()
        .find(|f| xref_is(&f.common, "F0001"))
        .expect("family");
    assert_eq!(fam.partner1, Some(person(&t, "I0001").common.id));
    assert_eq!(fam.partner2, Some(person(&t, "I0002").common.id));
    assert_eq!(fam.kind, UnionKind::Married);
    assert_eq!(fam.children.len(), 2);
    let c = &fam.children[0];
    assert_eq!(c.person, person(&t, "I0003").common.id);
    assert_eq!(
        (c.rel1.clone(), c.rel2.clone()),
        (ParentRel::Birth, ParentRel::Adopted)
    );
    assert!(c.preferred);
    assert_eq!(c.citations.len(), 1);
    assert_eq!(fam.children[1].rel1, ParentRel::Step);
    assert_eq!(
        fam.children[1].rel2,
        ParentRel::Other("Sponsored".to_string())
    );
    assert_eq!(fam.common.notes.len(), 1);
    assert!(fam.common.unknown.iter().any(|r| r.tag == "attribute"));

    let f2 = t
        .families
        .values()
        .find(|f| xref_is(&f.common, "F0002"))
        .expect("family 2");
    assert_eq!(f2.kind, UnionKind::CivilUnion);
    assert_eq!(f2.partner1, Some(person(&t, "I0004").common.id));
    assert_eq!(f2.partner2, None);
}

#[test]
fn sources_citations_notes_repositories() {
    let t = run(FIXTURE).tree;
    let s = t.sources.values().next().expect("source");
    assert_eq!(s.title.as_deref(), Some("Parish Register"));
    assert_eq!(s.author.as_deref(), Some("The Vicar"));
    assert_eq!(s.publication.as_deref(), Some("Parish Press"));
    assert_eq!(s.abbreviation.as_deref(), Some("PR"));
    assert_eq!(s.repositories.len(), 1);
    assert_eq!(s.repositories[0].call_number.as_deref(), Some("A-12"));
    let r = t.repositories.values().next().expect("repo");
    assert_eq!(r.name, "County Archive");
    assert_eq!(r.address.as_deref(), Some("2 Oak Rd, Springfield, USA"));
    assert_eq!(r.urls, vec!["https://example.org/archive"]);
    let c = t.citations.values().next().expect("citation");
    assert_eq!(c.source, Some(s.common.id));
    assert_eq!(c.page.as_deref(), Some("p. 12"));
    assert_eq!(c.quality, Some(3));
    assert!(matches!(
        c.date.as_ref().map(|d| &d.value),
        Some(DateValue::Before(_))
    ));
    assert_eq!(c.common.notes.len(), 1);

    let n1 = t
        .notes
        .values()
        .find(|n| xref_is(&n.common, "N0001"))
        .expect("note");
    assert_eq!(n1.text, "A shared note & more.");
    assert_eq!(n1.kind, NoteKind::General);
    assert!(n1.shared, "referenced by several records");
    let n2 = t
        .notes
        .values()
        .find(|n| xref_is(&n.common, "N0002"))
        .expect("note 2");
    assert_eq!(n2.kind, NoteKind::Research);
    assert!(!n2.shared);
    assert!(n2.common.unknown.iter().any(|r| r.tag == "style"));
}

#[test]
fn media_links_regions_and_resolution() {
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::create_dir_all(dir.path().join("photos")).expect("mkdir");
    std::fs::write(dir.path().join("photos/portrait.jpg"), b"jpg").expect("write");
    let opts = ImportOptions {
        gedcom_dir: Some(dir.path().to_path_buf()),
        ..ImportOptions::default()
    };
    let out = import_gramps(FIXTURE.as_bytes(), &opts).expect("import");
    let t = &out.tree;
    let m = t
        .media
        .values()
        .find(|m| xref_is(&m.common, "O0001"))
        .expect("media");
    assert_eq!(m.path, "photos/portrait.jpg");
    assert_eq!(m.mime.as_deref(), Some("image/jpeg"));
    assert_eq!(m.title.as_deref(), Some("Portrait"));
    assert_eq!(m.original_name.as_deref(), Some("portrait.jpg"));
    assert!(m.date.is_some());
    assert_eq!(out.resolved_media.len(), 1);
    assert_eq!(out.resolved_media[0].0, m.common.id);
    assert!(out.resolved_media[0].1.ends_with("portrait.jpg"));
    // the other file is listed as missing
    assert_eq!(out.report.missing_media.len(), 1);
    assert_eq!(out.report.missing_media[0].reason, MissingReason::NotFound);

    let link = &person(t, "I0001").common.media[0];
    assert_eq!(link.media, m.common.id);
    assert!(link.primary);
    let r = link.region.as_ref().expect("region");
    assert!((r.x - 0.1).abs() < 1e-6 && (r.y - 0.2).abs() < 1e-6);
    assert!((r.w - 0.5).abs() < 1e-6 && (r.h - 0.6).abs() < 1e-6);
}

#[test]
fn unmapped_things_are_reported_not_dropped() {
    let out = run(FIXTURE);
    let u = &out.report.unmapped_tags;
    for key in [
        "person/attribute",
        "person/address",
        "person/personref",
        "tagref",
        "family/attribute",
        "event/attribute",
        "note/style",
        "citation/srcattribute",
        "repository/type",
        "reporef/@medium",
        "file/@checksum",
        "database/bookmarks",
        "header/researcher",
        "header/mediapath",
    ] {
        assert!(u.contains_key(key), "missing {key}: {u:?}");
    }
    let p1 = person(&out.tree, "I0001");
    assert!(p1.common.unknown.iter().any(|r| r.tag == "attribute"));
    assert!(
        p1.common
            .unknown
            .iter()
            .any(|r| r.tag == "tagref" && r.value.as_deref() == Some("ToDo"))
    );
    // the dangling event reference
    assert!(
        out.report
            .repairs
            .iter()
            .any(|r| matches!(r, Repair::DanglingPointer { xref, .. } if xref == "_nope"))
    );
}

#[test]
fn duplicate_handles_and_cycles_are_repaired() {
    let xml = r##"<?xml version="1.0"?>
<database xmlns="http://gramps-project.org/xml/1.7.1/">
  <people>
    <person handle="_a" id="I1"><gender>M</gender></person>
    <person handle="_a" id="I2"><gender>F</gender></person>
  </people>
  <places>
    <placeobj handle="_x" id="P1"><pname value="X"/><placeref hlink="_y"/></placeobj>
    <placeobj handle="_y" id="P2"><pname value="Y"/><placeref hlink="_x"/></placeobj>
  </places>
</database>"##;
    let out = run(xml);
    assert_eq!(out.tree.persons.len(), 1);
    assert!(
        out.report
            .repairs
            .iter()
            .any(|r| matches!(r, Repair::DuplicateXref { xref, .. } if xref == "_a"))
    );
    assert_eq!(out.tree.places.len(), 2);
    assert!(out.tree.places.values().any(|p| p.parent.is_none()));
    assert!(out.report.unmapped_tags.contains_key("placeref(cycle)"));
}

#[test]
fn rejects_non_gramps_input() {
    let opts = ImportOptions::default();
    assert!(matches!(
        import_gramps(b"<html/>", &opts),
        Err(GrampsError::NotGramps(_))
    ));
    assert!(matches!(
        import_gramps(b"not xml at all", &opts),
        Err(GrampsError::Xml(_))
    ));
    assert!(matches!(
        import_gramps(&[0x1f, 0x8b, 0x00, 0x01], &opts),
        Err(GrampsError::Gzip(_))
    ));
    assert!(matches!(
        import_gramps(&[0xff, 0xfe, 0xfd], &opts),
        Err(GrampsError::NotUtf8)
    ));
}

fn wrap(inner: &str) -> String {
    format!(
        "<?xml version=\"1.0\"?><database xmlns=\"http://gramps-project.org/xml/1.7.1/\">{inner}</database>"
    )
}

#[test]
fn decompression_bomb_is_refused_without_inflating_it_all() {
    let mut e = GzEncoder::new(Vec::new(), Compression::best());
    e.write_all(b"<database>").expect("write");
    let chunk = vec![b' '; 1 << 20];
    for _ in 0..512 {
        e.write_all(&chunk).expect("write");
    }
    e.write_all(b"</database>").expect("write");
    let bomb = e.finish().expect("finish");
    assert!(bomb.len() < 1 << 20, "the bomb is small: {}", bomb.len());
    assert!(matches!(
        import_gramps(&bomb, &ImportOptions::default()),
        Err(GrampsError::TooLarge)
    ));
}

#[test]
fn truncated_gzip_is_an_error() {
    let mut data = gz(FIXTURE);
    data.truncate(data.len() / 2);
    assert!(matches!(
        import_gramps(&data, &ImportOptions::default()),
        Err(GrampsError::Gzip(_))
    ));
}

#[test]
fn hostile_media_paths_never_resolve_outside_the_roots() {
    let dir = tempfile::tempdir().expect("tmp");
    let root = dir.path().join("root");
    std::fs::create_dir_all(&root).expect("mkdir");
    std::fs::write(dir.path().join("secret.jpg"), b"x").expect("write");
    let outside = dir.path().join("secret.jpg");
    let abs = outside.to_string_lossy().replace(char::from(92), "/");
    let srcs = [
        abs.clone(),
        "../secret.jpg".to_string(),
        "//server/share/x.jpg".to_string(),
        format!("{0}{0}server{0}share{0}y.jpg", char::from(92)),
        format!("file:///{}", abs.trim_start_matches('/')),
    ];
    let objects: String = srcs
        .iter()
        .enumerate()
        .map(|(i, s)| {
            format!(
                "<object handle=\"_o{i}\" id=\"O{i}\"><file src=\"{s}\" mime=\"image/jpeg\"/></object>"
            )
        })
        .collect();
    let opts = ImportOptions {
        gedcom_dir: Some(root),
        ..ImportOptions::default()
    };
    let out = import_gramps(
        wrap(&format!("<objects>{objects}</objects>")).as_bytes(),
        &opts,
    )
    .expect("import");
    assert!(out.resolved_media.is_empty(), "{:?}", out.resolved_media);
    assert_eq!(out.report.missing_media.len(), srcs.len());
    assert!(out.report.missing_media.iter().all(|m| matches!(
        m.reason,
        MissingReason::OutsideRoots | MissingReason::NotFound
    )));
}

#[test]
fn malformed_dates_are_reported_not_fatal() {
    let ev = |i: u32, body: &str| {
        format!("<event handle=\"_e{i}\" id=\"E{i}\"><type>Birth</type>{body}</event>")
    };
    let inner = [
        ev(1, "<dateval val=\"garbage\"/>"),
        ev(2, "<dateval val=\"1900-13-01\"/>"),
        ev(3, "<dateval val=\"0000-00-00\"/>"),
        ev(4, "<daterange start=\"1900-01-01\" stop=\"\"/>"),
        ev(5, "<dateval val=\"1900-01-01\" type=\"wobble\"/>"),
        ev(6, "<daterange start=\"1950-01-01\" stop=\"1900-01-01\"/>"),
        ev(7, "<dateval val=\"2020-00-15\"/>"),
        ev(
            8,
            "<dateval val=\"1900-01-01\"/><dateval val=\"1901-01-01\"/>",
        ),
        ev(9, "<dateval val=\"-0044-03-15\"/>"),
    ]
    .concat();
    let out = run(&wrap(&format!("<events>{inner}</events>")));
    let u = &out.report.unmapped_tags;
    let t = &out.tree;
    assert!(matches!(date_of(t, "E1").value, DateValue::PhraseOnly));
    assert!(u.contains_key("date/value garbage"));
    assert!(matches!(date_of(t, "E2").value, DateValue::PhraseOnly));
    assert!(u.contains_key("date/month out of range"));
    assert!(event(t, "E3").date.is_none());
    assert!(matches!(date_of(t, "E4").value, DateValue::PhraseOnly));
    assert!(u.keys().any(|k| k.starts_with("date/value")));
    assert!(matches!(date_of(t, "E5").value, DateValue::PhraseOnly));
    assert!(u.contains_key("date/type wobble"));
    // start after stop is kept as written, never a panic
    assert!(matches!(date_of(t, "E6").value, DateValue::Between(..)));
    // day without month: the year survives and the loss is counted
    assert!(
        matches!(date_of(t, "E7").value, DateValue::Exact(ref d) if d.year == 2020 && d.day.is_none())
    );
    assert_eq!(u.get("date/day without month dropped"), Some(&1));
    assert!(u.contains_key("event/dateval(extra)"));
    assert!(matches!(date_of(t, "E9").value, DateValue::Exact(ref d) if d.bce && d.year == 44));
}

#[test]
fn dangling_handles_are_repaired_for_every_reference_kind() {
    let xml = wrap(
        r##"<events><event handle="_e1" id="E1"><type>Birth</type><place hlink="_noplace"/></event></events>
<people><person handle="_p1" id="I1"><gender>M</gender>
  <noteref hlink="_nonote"/><citationref hlink="_nocit"/><objref hlink="_noobj"/><tagref hlink="_notag"/>
  <childof hlink="_nofam"/><parentin hlink="_nofam2"/></person></people>
<families><family handle="_f1" id="F1"><father hlink="_nofather"/><mother hlink="_nomother"/>
  <childref hlink="_nochild"/></family></families>
<citations><citation handle="_c1" id="C1"><sourceref hlink="_nosrc"/><confidence>9</confidence></citation></citations>
<places><placeobj handle="_pl1" id="P1"><pname value="X"/><placeref hlink="_noparent"/></placeobj></places>"##,
    );
    let out = run(&xml);
    for h in [
        "_noplace",
        "_nonote",
        "_nocit",
        "_noobj",
        "_notag",
        "_nofam",
        "_nofam2",
        "_nofather",
        "_nomother",
        "_nochild",
        "_nosrc",
        "_noparent",
    ] {
        assert!(
            out.report
                .repairs
                .iter()
                .any(|r| matches!(r, Repair::DanglingPointer { xref, .. } if xref == h)),
            "no repair for {h}"
        );
    }
    let t = &out.tree;
    let f = t.families.values().next().expect("family");
    assert!(f.partner1.is_none() && f.partner2.is_none() && f.children.is_empty());
    assert!(t.citations.values().next().expect("c").source.is_none());
    assert!(t.places.values().next().expect("p").parent.is_none());
    assert_eq!(
        out.report.unmapped_tags.get("citation/confidence(invalid)"),
        Some(&1)
    );
}

#[test]
fn report_keys_are_bounded() {
    let many: String = (0..2000).map(|i| format!("<weird{i} a=\"1\"/>")).collect();
    let out = run(&wrap(&many));
    assert!(out.report.unmapped_tags.len() <= 301);
    assert!(
        out.report
            .unmapped_tags
            .keys()
            .all(|k| k.chars().count() <= 60)
    );
}
