/// Verbatim unknown data, format-neutral.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RawNode {
    pub tag: String,
    pub xref: Option<String>,
    pub value: Option<String>,
    /// Index among source siblings.
    pub pos: u32,
    pub children: Vec<RawNode>,
}
