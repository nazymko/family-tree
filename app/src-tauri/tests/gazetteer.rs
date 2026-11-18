#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! The one network use of the place data, against a local server and an invented country: the size from a HEAD
//! request, the download with progress, the index file it leaves, the cleanup, and a country the server lacks.

use gt_app_lib::gazetteer;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Mutex;

fn row(id: u32, name: &str, lat: &str, lon: &str, admin1: &str) -> String {
    // geonameid, name, asciiname, alternates, lat, lon, class, code, country, cc2, admin1, admin2, admin3, admin4, pop,
    // elevation, dem, timezone, modified
    format!(
        "{id}\t{name}\t{name}\t\t{lat}\t{lon}\tP\tPPL\tZZ\t\t{admin1}\t\t\t\t500\t\t\tEurope/Kyiv\t2020-01-01\n"
    )
}

fn zip_bytes() -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        z.start_file("ZZ.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        let text = [
            row(1, "Berezivka", "50.52", "30.28", "01"),
            row(2, "Dubky", "50.31", "31.05", "01"),
            row(3, "Hlukhiv", "51.2", "33.1", "02"),
        ]
        .concat();
        z.write_all(text.as_bytes()).unwrap();
        z.finish().unwrap();
    }
    buf.into_inner()
}

/// A server that answers `/ZZ.zip` and says 404 to anything else, for `requests` requests.
fn serve(requests: usize) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let body = zip_bytes();
    let h = std::thread::spawn(move || {
        for _ in 0..requests {
            let (mut s, _) = listener.accept().unwrap();
            let mut head = [0u8; 2048];
            let n = s.read(&mut head).unwrap();
            let req = String::from_utf8_lossy(&head[..n]).to_string();
            let ok = req.contains("/ZZ.zip");
            let is_head = req.starts_with("HEAD");
            let status = if ok { "200 OK" } else { "404 Not Found" };
            let len = if ok { body.len() } else { 0 };
            let _ = write!(
                s,
                "HTTP/1.1 {status}\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n"
            );
            if ok && !is_head {
                let _ = s.write_all(&body);
            }
            let _ = s.flush();
        }
    });
    (base, h)
}

#[test]
fn downloads_indexes_and_cleans_up() {
    let (base, server) = serve(3);
    let dir = tempfile::tempdir().unwrap();

    assert_eq!(
        gazetteer::remote_size_from(&base, "zz").unwrap(),
        Some(zip_bytes().len() as u64)
    );

    let seen = Mutex::new(Vec::new());
    let done = gazetteer::download_from(&base, dir.path(), "zz", &|p| seen.lock().unwrap().push(p))
        .unwrap();
    assert_eq!(done.country, "ZZ");
    assert_eq!(done.rows, 3);
    assert!(done.bytes > 0);

    let seen = seen.into_inner().unwrap();
    assert!(
        seen.iter()
            .any(|p| p.phase == "download" && p.done == p.total && p.total > 0)
    );
    assert_eq!(seen.last().unwrap().phase, "index");

    // The index is on disk, the zip is not.
    let folder = gazetteer::dir(dir.path());
    let names: Vec<String> = std::fs::read_dir(&folder)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(names, ["ZZ.gtgz"]);

    let st = gazetteer::status(dir.path());
    assert_eq!(st.countries.len(), 1);
    assert_eq!(st.countries[0].rows, 3);
    assert!(st.attribution.contains("GeoNames"));

    // The downloaded data answers a question.
    let gz = gazetteer::load(dir.path());
    let hits = gz.search(&["Berezivka", "Zelena oblast"], 5);
    assert_eq!(hits.first().map(|c| c.name.as_str()), Some("Berezivka"));

    // A country the server has no file for is a plain refusal and leaves nothing behind.
    let err = gazetteer::download_from(&base, dir.path(), "QQ", &|_| {}).unwrap_err();
    assert!(err.message.contains("QQ"), "{}", err.message);
    assert!(!gazetteer::dir(dir.path()).join("QQ.zip.part").exists());

    gazetteer::remove(dir.path(), "zz").unwrap();
    assert!(gazetteer::status(dir.path()).countries.is_empty());
    server.join().unwrap();
}

#[test]
fn a_country_is_two_letters() {
    for bad in ["", "U", "UKR", "../", "U1", "ua/"] {
        assert!(gazetteer::country_code(bad).is_err(), "{bad}");
    }
    assert_eq!(gazetteer::country_code(" ua ").unwrap(), "UA");
}
