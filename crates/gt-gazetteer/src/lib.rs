//! An offline gazetteer built from the GeoNames country dumps (CC-BY 4.0, https://www.geonames.org).
//!
//! - [`fold`]: the one normalisation used for the index and for queries (case, diacritics, Ukrainian to Latin).
//! - [`parse`]: a line of `<CC>.txt` to a [`parse::GeoRow`].
//! - [`index`]: the compact per-country file and the in-memory search tables.
//! - [`matcher`]: ranked candidates for a place typed as a hierarchy ("Village, District, Oblast").
//!
//! Nothing here uses the network: the app downloads the zip and hands it to [`index::Index::from_zip`].

pub mod fold;
pub mod index;
pub mod matcher;
pub mod parse;

pub use index::{Gazetteer, Index};
pub use matcher::Candidate;

/// The attribution GeoNames asks for; shown wherever the data is used.
pub const ATTRIBUTION: &str = "Place data: GeoNames (geonames.org), CC-BY 4.0";

#[derive(Debug, thiserror::Error)]
pub enum GazetteerError {
    #[error("could not read the file: {0}")]
    Io(#[from] std::io::Error),
    #[error("the zip file is damaged: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("the place index is damaged or from another version")]
    BadIndex,
    #[error("the country code must be two letters")]
    BadCountry,
    #[error("the file has no usable place rows")]
    Empty,
}
