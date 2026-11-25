# "Built around" chart: open side branches per person

Date: 2026-11-21. Status: approved approach (1), design for review.

## Why

The owner prints the "Built around" chart and wants more people on the print, but chosen ones. Today the
chart has one switch for everybody (`kin`: line / siblings / all): "siblings" adds only the centred person's
brothers and sisters, "all" adds the brothers and sisters of EVERY ancestor with all their descendants (65
people around one person in a real tree, most of them unwanted on paper). There is nothing in between.
The owner wants to pick a person on the chart (for example a great-grandfather) and open just that
person's brothers and sisters, or just their children, before printing.

## Goals

- On the "Built around" screen, every person on the chart can have their **brothers and sisters** opened,
  and every person off the main line can have their **children** opened.
- Each opening has a depth chosen per person:
  - **with children**: the opened people, their partners, and the opened people's children (one more row);
  - **with all descendants**: the opened people, their partners and all their descendants with partners.
- People who appear through an opening can be opened the same way (step by step).
- The print and the export show exactly what the screen shows.
- **Nothing opened = today's chart, byte for byte.** The existing `kin` switch keeps working as the base
  and is the default; the openings add to it.

## Non-goals

- No change to the pedigree, descendants or relatives charts.
- No saving openings in the project file or across app restarts.
- No new way to choose the people the chart is built around (Shift+click stays as it is).

## Anti-goals

- Boxes overlapping, or lines crossing through boxes, because of an opening.
- A print that differs from the screen.
- Any change in what an unchanged chart looks like (existing tests and snapshot charts must stay green
  unchanged).

## Approaches considered

1. **The core lays out the opened branches (chosen).** The chart request carries the list of openings;
   the layout engine in `gt-layout` hangs those families beside the existing boxes the way "all" already
   hangs every ancestor's family (`solve_all` in `crates/gt-layout/src/relatives.rs`), but only for the
   opened people and only to the chosen depth. The screen and the print send the same request
   (`ChartCanvas.svelte` builds it in one place), so they cannot differ.
2. Add each opened person as an extra person to build around, with `kin = siblings`. Rejected: it pulls in
   that person's whole ancestry and descendants, the opposite of a chosen print.
3. Draw the extra branches only in the screen. Rejected: the screen has no layout of its own, boxes would
   overlap, and the print would not have them.

## Design

### Request

`ChartRequest` (Rust `crates/gt-session/src/dto.rs`, generated TS `ChartRequest.ts`) gets an optional
field, used by the "around" kind only:

```rust
/// Chart built around people only: side branches opened per person. Absent or empty: the chart as before.
pub open: Option<Vec<OpenBranch>>,

pub struct OpenBranch {
    pub person: Uuid,
    /// The person's brothers and sisters (with their partners).
    pub siblings: Option<BranchDepth>,
    /// The person's children (with their partners); ignored for people whose children the chart already shows.
    pub children: Option<BranchDepth>,
}
pub enum BranchDepth { Children, All }  // ts: "children" | "all"
```

`None` / missing / empty list must take exactly today's code path (`around`, `around_with_siblings`,
`around_all_kin`, `around_many*`), so the default is unchanged by construction.

### Meaning

- **Siblings of P, depth Children**: P's full and half brothers and sisters (and an unnamed-siblings marker if
  P has one), each with partners, and each sibling's children with partners. Depth **All**: the same plus all
  further descendants with partners.
- **Children of P, depth Children**: P's children with partners. Depth **All**: all P's descendants with
  partners. Offered only for people whose children the chart does not already show (people off the line, and
  people that an opening showed with a shallower depth).
- Everyone already on the chart stays where the base `kin` puts them; an opening never removes anybody.
  Someone reached twice is drawn once (the usual duplicate rule of the chart).
- Openings of a person who is no longer on the chart (because an opening above them was closed) are kept
  in the list but have no effect; they come back when the person is shown again.
- Generations limit (`generations`) still applies to every row.

### Layout

Opened families hang like the "all" families: in their own generation rows, on the outer side of the person
they belong to (the father's line to the left, the mother's to the right; for people below the centre, the
side their parent's family is on), clearing everything already placed nearer the middle. The line keeps its
compact spine. Exactly how the hanging is generalised from `solve_all` (a filter of which families are hung
and how deep, versus a separate pass) is the builder's call, as long as the anti-goals hold.

### Screen

When a person's card is selected on the "Built around" screen, the bar that already shows "Build around this
person" and "Open person" gets two more controls:

```
Brothers & sisters:  [ off | with children | all descendants ]
Children:            [ off | with children | all descendants ]   (only when it applies)
```

- A card with something opened gets a small mark, so the openings can be found again.
- The openings live in the chart screen's state: kept while switching tabs and back, cleared when the chart
  is built around other people. "Clear all openings" sits next to the `kin` switch whenever any are set.
- English and Ukrainian strings (other locales fall back to English; filled later by the translation task).
- Keyboard: the controls are ordinary buttons in the selection bar; nothing new on the canvas.

### Print and export

They already rebuild the chart from the same request with `print: true`; the openings are part of that request.
The print dialog shows the number of people on the chart, so a large "all descendants" opening is visible before
printing.

### MCP

`get_around_chart` may take the same optional `open` list later; not part of this work.

## Decisions after the builder check (designer's calls, owner may override)

- **Scope of kin and chosen people.** Openings work when the chart is built around ONE person, for every
  `kin` value. With `kin = all` every ancestor's family is already fully shown, so the controls are offered
  only where they add something (people inside those families whose children are not shown, and in-laws if
  allowed, see below). When the chart is built around two or more people the controls are hidden and `open`
  is ignored (the `around_many*` paths stay untouched); a later step may extend it.
- **Who may be opened, and which control applies, is decided by the core.** Each node of an "around"
  chart gets two flags, `canOpenSiblings` and `canOpenChildren`, and the screen only reads them. Children are
  "already shown" for the centred person, every descendant on the line, and anyone an opening (or `kin`)
  already shows with all descendants.
- **Request identity.** People are given by `Uuid`; the core maps them to slots, ignores unknown ids and
  unnamed-sibling markers, and when one person appears twice the last entry wins. Opening the centred
  person's siblings is allowed (it behaves like `kin = siblings`, but with the chosen depth).
- **Limits.** `generations` and `MAX_NODES` still cut as today; when an opening is cut by the node limit, the
  chart's meta says so (`truncated` already exists for this) and the screen shows the existing "chart too
  large" notice.
- **"Nothing opened = today".** With `open` absent or `[]`, the chart meta, nodes, edges, relations, outline and
  print output are identical to today's; the open-chart cache key includes `open`.

## Tests (builder to extend)

- Every existing around-chart test and stored layout stays unchanged with `open` absent and with `open: []`.
- Opening siblings of an ancestor with depth Children: exactly those people appear, in the right rows, no
  overlaps (the existing layout quality checks).
- Depth All vs Children; children of an opened sibling; an opening under a closed one has no effect.
- The print request carries `open`; screen and print show the same people.
- UI: the controls show for a selected card, set the request, and are cleared when building around someone else.

## Left to the builder

- How `lay_out` / `solve_all` are generalised.
- The exact look of the controls and of the "opened" mark, within the app's existing selection-bar style.
- Where the people count sits in the print dialog.
