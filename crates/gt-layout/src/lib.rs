//! Chart layout for the gene-tree canvas views: a pedigree generation grid and a
//! Walker / Buchheim tidy tree for descendants, plus a uniform-grid viewport index.
//!
//! The crate knows nothing about `gt-core`: callers adapt their data through [`ChartSource`].
//! Both layouts use explicit work lists instead of recursion.

mod descendants;
mod grid;
mod pedigree;
mod types;

pub use descendants::descendants;
pub use pedigree::pedigree;
pub use types::{ChartSource, Edge, EdgeKind, Key, Layout, Metrics, Nav, Node, Rect};
