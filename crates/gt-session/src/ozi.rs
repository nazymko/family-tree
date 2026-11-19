//! Corners of a scanned map from its OziExplorer `.map` file (the georeference that comes with the
//! three-verst sheets and many other old maps). The file gives up to four points of the map's
//! frame twice: in pixels of the picture (`MMPXY`) and as longitude and latitude (`MMPLL`). The
//! picture's own corners follow from those by a projective transform (an affine one from three
//! points). A file without those lines gives its calibration points instead (`Point01` to
//! `Point30`: pixels, then degrees and minutes). Coordinates in the Pulkovo 1942 datum are moved to
//! WGS 84, as the Map draws.
//!
//! Both kinds of points are taken in the datum the file names. OziExplorer's own description of
//! the format names no datum for `MMPLL`; programs that write these files for the old Soviet sheets
//! (mapsoft2, for one) write `MMPLL` in the map's datum, and readers such as tilers-tools read them
//! so. A file on Pulkovo 1942 is therefore moved to WGS 84 whichever lines it is read from.
//!
//! The pixels of the file are those of the picture it was calibrated on (`IWH`). A picture of the
//! same shape but another size is that picture scaled, and gets the same corners. A picture of
//! another shape was cropped or framed differently (a site's JPEG next to its `.ozf2`): its corners
//! cannot be known from the file, and [`OziCorners::mismatch`] says so.

use crate::command::MapCorner;
use crate::error::{ApiError, ErrorCode};

/// The largest `.map` file read (they are a few kilobytes).
pub const OZI_MAP_MAX: u64 = 1024 * 1024;

/// What a `.map` file says of where a picture lies.
#[derive(Debug, Clone, PartialEq)]
pub struct OziCorners {
    /// Top left, top right, bottom right, bottom left of the picture, in WGS 84.
    pub corners: [MapCorner; 4],
    /// The datum the file names, as written.
    pub datum: String,
    /// The picture size the file names (`IWH`), or the one given.
    pub width: u32,
    pub height: u32,
    /// The picture given does not fit the file: another shape than the calibrated one, or points
    /// of the file outside it. The corners are then only a first guess for the owner to correct.
    pub mismatch: bool,
}

/// What an agent or the owner is told when a `.map` file does not fit its picture.
pub const OZI_MISMATCH: &str = "The .map file does not fit this picture: the picture has another shape than the one the .map file was made for (it was cropped or framed differently), so its corners would be kilometres off. Use the picture the .map file was made for, or give the corners.";

/// How far the shapes (width over height) of the picture and of the calibrated one may differ.
const SHAPE_TOLERANCE: f64 = 0.005;

impl OziCorners {
    /// The corners, or the refusal when the file does not fit the picture.
    pub fn fitting(&self) -> Result<[MapCorner; 4], ApiError> {
        if self.mismatch {
            Err(bad(OZI_MISMATCH))
        } else {
            Ok(self.corners)
        }
    }
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::Invalid, msg)
}

fn num(s: Option<&&str>) -> Option<f64> {
    s.and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite())
}

/// Reads an OziExplorer `.map` file. `dims` is the picture's size, used when the file names none.
pub fn read_ozi_map(text: &str, dims: Option<(u32, u32)>) -> Result<OziCorners, ApiError> {
    let mut lines = text.lines();
    let first = lines.next().unwrap_or_default();
    if !first
        .trim_start_matches('\u{feff}')
        .starts_with("OziExplorer Map Data File")
    {
        return Err(bad("This is not an OziExplorer .map file."));
    }
    let all: Vec<&str> = text.lines().collect();
    let datum = all
        .get(4)
        .and_then(|l| l.split(',').next())
        .unwrap_or_default()
        .trim()
        .to_string();
    let mut xy: [Option<(f64, f64)>; 4] = [None; 4];
    let mut ll: [Option<(f64, f64)>; 4] = [None; 4];
    let mut iwh: Option<(u32, u32)> = None;
    let mut calibration: Vec<((f64, f64), (f64, f64))> = Vec::new();
    for line in &all {
        let f: Vec<&str> = line.split(',').collect();
        let key = f.first().map(|k| k.trim()).unwrap_or_default();
        let slot = |i: Option<&&str>| {
            i.and_then(|v| v.trim().parse::<usize>().ok())
                .filter(|i| (1..=4).contains(i))
                .map(|i| i - 1)
        };
        match key {
            "MMPXY" => {
                if let (Some(i), Some(x), Some(y)) = (slot(f.get(1)), num(f.get(2)), num(f.get(3)))
                    && let Some(s) = xy.get_mut(i)
                {
                    *s = Some((x, y));
                }
            }
            "MMPLL" => {
                if let (Some(i), Some(lon), Some(lat)) =
                    (slot(f.get(1)), num(f.get(2)), num(f.get(3)))
                    && let Some(s) = ll.get_mut(i)
                {
                    *s = Some((lon, lat));
                }
            }
            k if k.len() == 7 && k.starts_with("Point") => {
                if let Some(p) = calibration_point(&f) {
                    calibration.push(p);
                }
            }
            "IWH" => {
                let w = num(f.get(2)).filter(|v| *v >= 1.0);
                let h = num(f.get(3)).filter(|v| *v >= 1.0);
                if let (Some(w), Some(h)) = (w, h) {
                    iwh = Some((w as u32, h as u32));
                }
            }
            _ => {}
        }
    }
    let corner_pairs: Vec<((f64, f64), (f64, f64))> = xy
        .iter()
        .zip(&ll)
        .filter_map(|(a, b)| Some(((*a)?, (*b)?)))
        .collect();
    let pairs = if corner_pairs.len() >= 3 {
        corner_pairs.clone()
    } else {
        calibration.clone()
    };
    if pairs.len() < 3 {
        return Err(bad(
            "The .map file names fewer than three corner points (MMPXY and MMPLL lines).",
        ));
    }
    if pairs
        .iter()
        .any(|(_, (lon, lat))| !(-180.0..=180.0).contains(lon) || !(-90.0..=90.0).contains(lat))
    {
        return Err(bad("A point of the .map file is off the Earth."));
    }
    let (width, height) = iwh.or(dims).ok_or_else(|| {
        bad("The .map file does not say how big the picture is, and the picture cannot be read.")
    })?;
    let mismatch = {
        let (w, h) = (f64::from(width), f64::from(height));
        let shape = |(a, b): (u32, u32)| f64::from(a) / f64::from(b);
        let other_shape = dims.is_some_and(|d| {
            (shape(d) - shape((width, height))).abs() / shape((width, height)) > SHAPE_TOLERANCE
        });
        // A point a little off the edge is a frame drawn to the edge; one well outside is not on
        // this picture at all.
        let (mx, my) = (w * SHAPE_TOLERANCE, h * SHAPE_TOLERANCE);
        let outside = corner_pairs
            .iter()
            .chain(&calibration)
            .any(|((x, y), _)| *x < -mx || *y < -my || *x > w + mx || *y > h + my);
        other_shape || outside
    };
    let to_ll =
        transform(&pairs).ok_or_else(|| bad("The points of the .map file lie on one line."))?;
    let (w, h) = (f64::from(width), f64::from(height));
    let shift = datum_shift(&datum);
    let mut corners = [MapCorner {
        lat_e6: 0,
        lon_e6: 0,
    }; 4];
    for (c, (x, y)) in corners
        .iter_mut()
        .zip([(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)])
    {
        let (lon, lat) = to_ll(x, y);
        let (lon, lat) = match shift {
            Some(d) => molodensky(lon, lat, d),
            None => (lon, lat),
        };
        if !(-180.0..=180.0).contains(&lon) || !(-90.0..=90.0).contains(&lat) {
            return Err(bad("A corner of the picture falls off the Earth."));
        }
        *c = MapCorner {
            lat_e6: (lat * 1e6).round() as i32,
            lon_e6: (lon * 1e6).round() as i32,
        };
    }
    Ok(OziCorners {
        corners,
        datum,
        width,
        height,
        mismatch,
    })
}

/// A calibration line: `Point01,xy, 494, 235,in, deg, 24, 0,S, 148, 0,E, grid, ...`, the pixel
/// and then latitude and longitude in degrees, minutes and hemisphere. An unused line (no pixel or
/// no degrees) gives nothing.
fn calibration_point(f: &[&str]) -> Option<((f64, f64), (f64, f64))> {
    let x = num(f.get(2))?;
    let y = num(f.get(3))?;
    let angle = |d: usize, minus: &str| {
        let deg = num(f.get(d))?;
        let min = num(f.get(d + 1)).unwrap_or(0.0);
        let v = deg.abs() + min / 60.0;
        let neg = deg < 0.0
            || f.get(d + 2)
                .is_some_and(|h| h.trim().eq_ignore_ascii_case(minus));
        Some(if neg { -v } else { v })
    };
    let lat = angle(6, "S")?;
    let lon = angle(9, "W")?;
    Some(((x, y), (lon, lat)))
}

/// Pixel to (lon, lat): projective through four pairs, affine through three.
#[allow(clippy::type_complexity)]
fn transform(pairs: &[((f64, f64), (f64, f64))]) -> Option<Box<dyn Fn(f64, f64) -> (f64, f64)>> {
    if pairs.len() >= 4 {
        // h11 x + h12 y + h13 - h31 x X - h32 y X = X, and the same for Y; h33 = 1.
        let mut a = vec![vec![0.0; 9]; 8];
        for (i, ((x, y), (u, v))) in pairs.iter().take(4).enumerate() {
            a[2 * i] = vec![*x, *y, 1.0, 0.0, 0.0, 0.0, -x * u, -y * u, *u];
            a[2 * i + 1] = vec![0.0, 0.0, 0.0, *x, *y, 1.0, -x * v, -y * v, *v];
        }
        let h = solve(a)?;
        return Some(Box::new(move |x, y| {
            let d = h[6] * x + h[7] * y + 1.0;
            (
                (h[0] * x + h[1] * y + h[2]) / d,
                (h[3] * x + h[4] * y + h[5]) / d,
            )
        }));
    }
    let mut a = vec![vec![0.0; 7]; 6];
    for (i, ((x, y), (u, v))) in pairs.iter().take(3).enumerate() {
        a[2 * i] = vec![*x, *y, 1.0, 0.0, 0.0, 0.0, *u];
        a[2 * i + 1] = vec![0.0, 0.0, 0.0, *x, *y, 1.0, *v];
    }
    let h = solve(a)?;
    Some(Box::new(move |x, y| {
        (h[0] * x + h[1] * y + h[2], h[3] * x + h[4] * y + h[5])
    }))
}

/// Gaussian elimination with partial pivoting on an augmented n x (n + 1) matrix.
#[allow(clippy::indexing_slicing, clippy::needless_range_loop)]
fn solve(mut a: Vec<Vec<f64>>) -> Option<Vec<f64>> {
    let n = a.len();
    // Pixels are thousands while degrees are tens: scale each row by its largest entry.
    for row in a.iter_mut() {
        let m = row.iter().take(n).fold(0.0_f64, |m, v| m.max(v.abs()));
        if m > 0.0 {
            row.iter_mut().for_each(|v| *v /= m);
        }
    }
    for c in 0..n {
        let p = (c..n).max_by(|i, j| a[*i][c].abs().total_cmp(&a[*j][c].abs()))?;
        if a[p][c].abs() < 1e-12 {
            return None;
        }
        a.swap(c, p);
        for r in 0..n {
            if r != c {
                let k = a[r][c] / a[c][c];
                for j in c..=n {
                    a[r][j] -= k * a[c][j];
                }
            }
        }
    }
    Some((0..n).map(|i| a[i][n] / a[i][i]).collect())
}

/// Datum to WGS 84: dx, dy, dz in metres, and the source ellipsoid's a and 1/f.
#[derive(Debug, Clone, Copy)]
struct Shift {
    dx: f64,
    dy: f64,
    dz: f64,
    a: f64,
    inv_f: f64,
}

/// The shift for the datums old maps of this part of the world are drawn on; none for WGS 84
/// and for a datum this reader does not know.
fn datum_shift(datum: &str) -> Option<Shift> {
    let d = datum.to_ascii_lowercase();
    let krassovsky = |dx, dy, dz| Shift {
        dx,
        dy,
        dz,
        a: 6_378_245.0,
        inv_f: 298.3,
    };
    if d.starts_with("pulkovo 1942 (2)") {
        Some(krassovsky(23.92, -141.27, -80.9))
    } else if d.starts_with("pulkovo") || d.starts_with("s42") || d.starts_with("s-42") {
        Some(krassovsky(28.0, -130.0, -95.0))
    } else {
        None
    }
}

/// The abridged Molodensky transformation (good to a few metres), height 0.
fn molodensky(lon: f64, lat: f64, s: Shift) -> (f64, f64) {
    const WGS_A: f64 = 6_378_137.0;
    const WGS_F: f64 = 1.0 / 298.257_223_563;
    let f = 1.0 / s.inv_f;
    let (da, df) = (WGS_A - s.a, WGS_F - f);
    let e2 = 2.0 * f - f * f;
    let (phi, lam) = (lat.to_radians(), lon.to_radians());
    let (sp, cp, sl, cl) = (phi.sin(), phi.cos(), lam.sin(), lam.cos());
    let w = (1.0 - e2 * sp * sp).sqrt();
    let rm = s.a * (1.0 - e2) / (w * w * w);
    let rn = s.a / w;
    let dphi =
        (-s.dx * sp * cl - s.dy * sp * sl + s.dz * cp + (s.a * df + f * da) * (2.0 * phi).sin())
            / rm;
    let dlam = (-s.dx * sl + s.dy * cl) / (rn * cp);
    ((lam + dlam).to_degrees(), (phi + dphi).to_degrees())
}

/// Reads a `.map` file from disk (at most [`OZI_MAP_MAX`] bytes; any 8-bit encoding).
pub fn read_ozi_map_file(
    path: &std::path::Path,
    dims: Option<(u32, u32)>,
) -> Result<OziCorners, ApiError> {
    let meta = std::fs::metadata(path).map_err(|_| bad("The .map file cannot be read."))?;
    if !meta.is_file() || meta.len() > OZI_MAP_MAX {
        return Err(bad("The .map file is not a small text file."));
    }
    let bytes = std::fs::read(path).map_err(|_| bad("The .map file cannot be read."))?;
    read_ozi_map(&String::from_utf8_lossy(&bytes), dims)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;

    /// A synthetic sheet: the frame at (100,100)-(900,100)-(900,700)-(100,700) in a 1000 x 800
    /// picture, the frame's corners a plain longitude/latitude rectangle.
    const MAP: &str = "OziExplorer Map Data File Version 2.2\r\nsheet.jpg\r\nC:\\x\\sheet.ozf2\r\n1 ,Map Code,\r\nWGS 84,WGS 84,   0.0000,   0.0000,WGS 84\r\nReserved 1\r\nReserved 2\r\nMagnetic Variation,,,E\r\nMap Projection,Latitude/Longitude,PolyCal,No,AutoCalOnly,No,BSBUseWPX,No\r\nMMPNUM,4\r\nMMPXY,1,100,100\r\nMMPXY,2,900,100\r\nMMPXY,3,900,700\r\nMMPXY,4,100,700\r\nMMPLL,1,  30.000000,  50.600000\r\nMMPLL,2,  30.800000,  50.600000\r\nMMPLL,3,  30.800000,  50.000000\r\nMMPLL,4,  30.000000,  50.000000\r\nIWH,Map Image Width/Height,1000,800\r\n";

    #[test]
    fn the_picture_corners_follow_from_the_frame() {
        let o = read_ozi_map(MAP, None).unwrap();
        assert_eq!((o.width, o.height), (1000, 800));
        assert_eq!(o.datum, "WGS 84");
        // 800 px = 0.8 degrees of longitude, 600 px = 0.6 of latitude: 100 px is 0.1 degree.
        let c = o.corners;
        assert_eq!((c[0].lon_e6, c[0].lat_e6), (29_900_000, 50_700_000));
        assert_eq!((c[1].lon_e6, c[1].lat_e6), (30_900_000, 50_700_000));
        assert_eq!((c[2].lon_e6, c[2].lat_e6), (30_900_000, 49_900_000));
        assert_eq!((c[3].lon_e6, c[3].lat_e6), (29_900_000, 49_900_000));
    }

    /// The .map is calibrated on the site's own copy of the picture (IWH 1000 x 800 here); the
    /// downloaded JPEG of the same shape is that picture scaled, so its corners are the corners the
    /// .map's own picture has, whatever size the JPEG is.
    #[test]
    fn a_picture_of_another_size_gets_the_corners_of_the_calibrated_one() {
        let calibrated = read_ozi_map(MAP, None).unwrap();
        let bigger = read_ozi_map(MAP, Some((1450, 1160))).unwrap();
        assert_eq!(bigger.corners, calibrated.corners);
        assert_eq!((bigger.width, bigger.height), (1000, 800));
        assert!(!bigger.mismatch);
        assert_eq!(bigger.fitting().unwrap(), calibrated.corners);
        // Scaled by hand: the frame's top left (100, 100) of the .map is (145, 145) of the JPEG,
        // and the JPEG's corner (0, 0) is 0.1 degree out from it either way.
        assert_eq!(
            (bigger.corners[0].lon_e6, bigger.corners[0].lat_e6),
            (29_900_000, 50_700_000)
        );
        // Rounding a pixel off when it was scaled is no mismatch.
        assert!(!read_ozi_map(MAP, Some((1001, 800))).unwrap().mismatch);
    }

    /// A JPEG cropped from the calibrated picture (a site's JPEG next to its .ozf2, framed
    /// otherwise) has another shape: stretching the .map onto it put sheets 7 to 12 km off. The
    /// file is said not to fit, and an agent is refused.
    #[test]
    fn a_cropped_picture_does_not_fit() {
        // The 1000 x 800 sheet with 60 px cut off the right: 940 x 800.
        let cropped = read_ozi_map(MAP, Some((940, 800))).unwrap();
        assert!(cropped.mismatch);
        assert_eq!(cropped.fitting().unwrap_err().message, OZI_MISMATCH);
        // Another shape also when the size is scaled as well.
        assert!(read_ozi_map(MAP, Some((1450, 1104))).unwrap().mismatch);
    }

    /// The pixels of the file fall outside the picture it names: it was made for another frame.
    #[test]
    fn points_outside_the_picture_do_not_fit() {
        let off = MAP.replace("MMPXY,2,900,100", "MMPXY,2,1300,100");
        let o = read_ozi_map(&off, None).unwrap();
        assert!(o.mismatch);
        assert!(o.fitting().is_err());
    }

    /// A file with no MMPLL lines is read from its calibration points (degrees and minutes, in the
    /// map's datum).
    #[test]
    fn calibration_points_stand_in_for_the_corner_lines() {
        let points = "Point01,xy,  100,  100,in, deg,  50, 36.000000,N,  30,  0.000000,E, grid,   ,           ,           ,N\r\n\
                      Point02,xy,  900,  100,in, deg,  50, 36.000000,N,  30, 48.000000,E, grid,   ,           ,           ,N\r\n\
                      Point03,xy,  900,  700,in, deg,  50,  0.000000,N,  30, 48.000000,E, grid,   ,           ,           ,N\r\n\
                      Point04,xy,     ,     ,in, deg,    ,          ,N,    ,          ,E, grid,   ,           ,           ,N\r\n";
        let map: String = MAP
            .lines()
            .filter(|l| !l.starts_with("MMPLL"))
            .map(|l| {
                if l.starts_with("Map Projection") {
                    format!("{l}\r\n{points}")
                } else {
                    format!("{l}\r\n")
                }
            })
            .collect();
        let o = read_ozi_map(&map, None).unwrap();
        assert_eq!(o.corners, read_ozi_map(MAP, None).unwrap().corners);
        // Southern and western hemispheres turn the sign.
        let south = calibration_point(
            &"Point01,xy,1,2,in,deg,24,30,S,148,15,W,grid"
                .split(',')
                .collect::<Vec<_>>(),
        )
        .unwrap();
        assert_eq!(south, ((1.0, 2.0), (-148.25, -24.5)));
        // Pulkovo 1942 is moved to WGS 84 from the calibration points just as from MMPLL.
        let pulkovo = |m: &str| {
            read_ozi_map(
                &m.replacen("WGS 84,WGS 84", "Pulkovo 1942 (1),WGS 84", 1),
                None,
            )
            .unwrap()
            .corners
        };
        assert_eq!(pulkovo(&map), pulkovo(MAP));
        assert_ne!(pulkovo(MAP), o.corners);
    }

    #[test]
    fn three_points_are_enough_and_the_size_may_come_from_the_picture() {
        let three: String = MAP
            .lines()
            .filter(|l| {
                !l.starts_with("MMPXY,4") && !l.starts_with("MMPLL,4") && !l.starts_with("IWH")
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(read_ozi_map(&three, None).is_err(), "no size anywhere");
        let o = read_ozi_map(&three, Some((1000, 800))).unwrap();
        assert_eq!(
            (o.corners[2].lon_e6, o.corners[2].lat_e6),
            (30_900_000, 49_900_000)
        );
    }

    #[test]
    fn pulkovo_coordinates_are_moved_to_wgs84() {
        let map = MAP.replacen("WGS 84,WGS 84", "Pulkovo 1942 (1),WGS 84", 1);
        let o = read_ozi_map(&map, None).unwrap();
        let plain = read_ozi_map(MAP, None).unwrap();
        // Around 50 N, 30 E the two datums differ by roughly 100 to 150 metres, mostly in longitude.
        let dlon = f64::from(o.corners[0].lon_e6 - plain.corners[0].lon_e6) / 1e6;
        let dlat = f64::from(o.corners[0].lat_e6 - plain.corners[0].lat_e6) / 1e6;
        let metres = ((dlon * 71_000.0).powi(2) + (dlat * 111_000.0).powi(2)).sqrt();
        assert!((50.0..250.0).contains(&metres), "{metres} m");
    }

    /// A local check on real `.map` files: `GT_OZI_DIR=<folder> cargo test -p gt-session --lib
    /// ozi::tests::local -- --ignored --nocapture` prints the corners of every one.
    #[test]
    #[ignore = "reads a local folder named by GT_OZI_DIR"]
    fn local_folder_of_map_files() {
        let Ok(dir) = std::env::var("GT_OZI_DIR") else {
            return;
        };
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "map") {
                let o = read_ozi_map_file(&p, None).unwrap();
                let c: Vec<String> = o
                    .corners
                    .iter()
                    .map(|c| {
                        format!(
                            "{:.4} {:.4}",
                            f64::from(c.lat_e6) / 1e6,
                            f64::from(c.lon_e6) / 1e6
                        )
                    })
                    .collect();
                println!(
                    "{} {} {}x{}: {}",
                    p.display(),
                    o.datum,
                    o.width,
                    o.height,
                    c.join(" | ")
                );
            }
        }
    }

    #[test]
    fn anything_else_is_refused() {
        assert!(read_ozi_map("hello", Some((10, 10))).is_err());
        let few = MAP.replace("MMPLL,2", "XX").replace("MMPLL,3", "XX");
        assert!(read_ozi_map(&few, None).is_err());
        let off = MAP.replace("50.600000", "95.000000");
        assert!(read_ozi_map(&off, None).is_err());
    }
}
