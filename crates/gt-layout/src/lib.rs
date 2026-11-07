//! Chart layout for the gene-tree canvas views: a pedigree generation grid and a
//! Walker / Buchheim tidy tree for descendants, plus a uniform-grid viewport index.
//!
//! The crate knows nothing about `gt-core`: callers adapt their data through [`ChartSource`].
//! The relatives chart is an hourglass of both. All layouts use explicit work lists instead of recursion.

mod descendants;
mod grid;
mod pedigree;
mod quality;
mod relatives;
mod types;
mod whole;

pub use descendants::descendants;
pub use pedigree::pedigree;
pub use quality::{Quality, quality};
pub use relatives::{
    ManyKin, RelativesOptions, around, around_all_kin, around_many, around_many_kin,
    around_with_siblings, relatives,
};
pub use types::{
    Axis, ChartSource, Edge, EdgeKind, Key, Layout, Metrics, Nav, Node, PartnerOf, Rect, Rel,
};
pub use whole::{
    Block, BlockKind, WholeExtras, WholeFamily, WholeInput, WholeLayout, whole, whole_directed,
};
