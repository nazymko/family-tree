//! A synthetic `.atdb` database for tests (feature `fixture`). All names are invented.
//!
//! Three generations: Ivan Petrovych Kovalenko and Maria Hryhorivna (born Shevchenko), their son
//! Petro Ivanovych and his wife Olha Stepanivna (born Melnyk), and their son Taras Petrovych.
//! It also has a date range, a year-only date, a month-and-year date, a place hierarchy, an
//! unknown event type, an unknown date qualifier, an unknown person field and a name variant.

use rusqlite::Connection;
use std::path::Path;

/// The schema of a Древо Жизни database (schema version 8), triggers included.
pub const SCHEMA: &str = include_str!("../atdb-schema.sql");

/// Creates the family database at `path` (which must not exist yet). Returns the connection so a
/// test can add more rows before closing it.
pub fn build_family_fixture(path: &Path) -> Connection {
    let conn = Connection::open(path).expect("create fixture database");
    // The fixture does not carry the Fields / EventTypes lookup rows the foreign keys name.
    conn.execute_batch("PRAGMA foreign_keys = OFF")
        .expect("pragma");
    // The dump lists the triggers before the tables they watch; create the tables first.
    let (triggers, tables) = SCHEMA.split_at(SCHEMA.find("CREATE TABLE").expect("tables"));
    conn.execute_batch(tables).expect("apply tables");
    conn.execute_batch(triggers).expect("apply triggers");
    conn.execute_batch(DATA).expect("insert fixture rows");
    conn
}

const DATA: &str = "
INSERT INTO Global (id, version, guid, mainlang) VALUES (1, 8, 'g', 'uk');
INSERT INTO EventTypes (id, ord) VALUES (1,1),(2,2),(3,3),(7,7);
INSERT INTO Persons (id, sex) VALUES (1,1),(2,2),(3,1),(4,2),(5,1);
INSERT INTO Places (id, parent_id) VALUES (1, NULL), (2, 1);
-- person names: 66 given, 67 patronymic, 64 surname, 65 maiden
INSERT INTO ValuesStr (f_id, rec_table, rec_id, lang, vstr) VALUES
 (66,13,1,NULL,'Ivan'),   (67,13,1,NULL,'Petrovych'),   (64,13,1,NULL,'Kovalenko'),
 (66,13,2,NULL,'Maria'),  (67,13,2,NULL,'Hryhorivna'),  (64,13,2,NULL,'Kovalenko'), (65,13,2,NULL,'Shevchenko'),
 (66,13,3,NULL,'Petro'),  (67,13,3,NULL,'Ivanovych'),   (64,13,3,NULL,'Kovalenko'),
 (66,13,4,NULL,'Olha'),   (67,13,4,NULL,'Stepanivna'),  (64,13,4,NULL,'Kovalenko'), (65,13,4,NULL,'Melnyk'),
 (66,13,5,'uk','Taras'),  (67,13,5,'uk','Petrovych'),   (64,13,5,'uk','Kovalenko'),
 (66,13,5,'en','Taras'),  (64,13,5,'en','Kovalenko'),
 (69,13,5,NULL,'a field nobody maps'),
 (93,14,1,NULL,'Kyiv Oblast'), (94,14,1,NULL,'Kyiv Oblast'),
 (93,14,2,NULL,'Village, Kyiv Oblast'), (94,14,2,NULL,'Village');
INSERT INTO Events (id, et_id) VALUES (1,1),(2,2),(3,1),(4,3),(5,1),(6,1),(7,3),(8,1),(9,7);
-- roles: 1 child, 2 father, 3 mother, 4 deceased, 5/6 spouses, 30 = main role of event type 7
INSERT INTO EventDetails (p_id, e_id, er_id, e_ord, p_ord) VALUES
 (1,1,1,1,1), (1,2,4,2,1), (2,3,1,1,1), (1,4,5,3,1), (2,4,6,3,2),
 (3,5,1,1,1), (1,5,2,1,2), (2,5,3,1,3),
 (4,6,1,1,1), (3,7,5,2,1), (4,7,6,2,2),
 (5,8,1,1,1), (3,8,2,1,2), (4,8,3,1,3),
 (5,9,30,2,1);
-- dates (field 29): d m y d2 m2 y2 type
INSERT INTO ValuesDates (f_id, rec_table, rec_id, d, m, y, d2, m2, y2, type) VALUES
 (29,7,1,NULL,NULL,1900,NULL,NULL,NULL,0),
 (29,7,2,5,11,1970,NULL,NULL,NULL,0),
 (29,7,3,NULL,NULL,1903,NULL,NULL,NULL,0),
 (29,7,4,NULL,NULL,1924,NULL,NULL,1925,0),
 (29,7,5,12,3,1926,NULL,NULL,NULL,0),
 (29,7,6,NULL,NULL,1930,NULL,NULL,NULL,2),
 (29,7,7,NULL,NULL,1950,NULL,NULL,NULL,0),
 (29,7,8,NULL,6,1952,NULL,NULL,NULL,0);
-- place of event 5 (field 28) is place 2
INSERT INTO ValuesLinks (f_id, rec_table, rec_id, vlink_table, vlink_id) VALUES (28,7,5,14,2);
";
