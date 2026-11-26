//! Off-sheet cross references (#1273): for each Global label of the stitched
//! project, the other pages its net continues on through a Global label, so a
//! printed schematic can be followed from page to page.
//!
//! Read from the net index ([`NetIndex`]); nothing here re-derives
//! connectivity. Page numbers come from the project numbering
//! ([`SheetNumbering`]), the one the title block and the PDF print.

use std::collections::HashSet;

use signex_types::schematic::{InstancePath, LabelType, Point};
use uuid::Uuid;

use super::SheetKey;
use super::members::{NetMember, NetMemberKind};
use super::net_index::NetIndex;
use super::sheet_numbers::SheetNumbering;

/// One Global label on one page and where its net continues.
#[derive(Debug, Clone, PartialEq)]
pub struct PortCrossRef {
    pub sheet: SheetKey,
    /// The placement of `sheet` this label is printed on.
    pub occurrence: InstancePath,
    pub label: Uuid,
    /// The label's text, as it sits on the net.
    pub text: String,
    /// The page number the label is printed on, when the numbering has it.
    pub page: Option<usize>,
    /// Global labels of the same net on other pages, by page number, then
    /// left to right and top to bottom. Empty when the net stays on this page.
    pub partners: Vec<CrossRefPartner>,
}

/// A Global label on another page, on the same net as the label it is listed
/// for.
#[derive(Debug, Clone, PartialEq)]
pub struct CrossRefPartner {
    pub sheet: SheetKey,
    pub occurrence: InstancePath,
    pub label: Uuid,
    /// The page number it is printed on.
    pub page: usize,
    /// Where it sits on its sheet, in mm.
    pub at: Point,
}

impl NetIndex {
    /// Every Global label of the walk, once per placement, in net then walk
    /// order, with its partners on other pages. A partner on a page the
    /// numbering does not reach has no number to print and is left out, as is
    /// one on the label's own page.
    pub fn port_cross_refs(&self, numbering: &SheetNumbering) -> Vec<PortCrossRef> {
        let mut out = Vec::new();
        let mut seen: HashSet<(SheetKey, InstancePath, Uuid)> = HashSet::new();
        for index in 0..self.net_count() {
            let Some(net) = self.net(index) else {
                continue;
            };
            let globals: Vec<&NetMember> = net
                .members
                .iter()
                .filter(|m| m.kind == NetMemberKind::Label(LabelType::Global))
                .collect();
            for source in &globals {
                let first =
                    seen.insert((source.sheet.clone(), source.occurrence.clone(), source.id));
                if !first {
                    continue;
                }
                let page = numbering.number_of(&source.sheet, &source.occurrence);
                out.push(PortCrossRef {
                    sheet: source.sheet.clone(),
                    occurrence: source.occurrence.clone(),
                    label: source.id,
                    text: source.text.clone(),
                    page,
                    partners: partners_of(source, page, &globals, numbering),
                });
            }
        }
        out
    }
}

/// The Global labels of `globals` on pages other than `source`'s, sorted.
fn partners_of(
    source: &NetMember,
    source_page: Option<usize>,
    globals: &[&NetMember],
    numbering: &SheetNumbering,
) -> Vec<CrossRefPartner> {
    let mut seen: HashSet<(SheetKey, InstancePath, Uuid)> = HashSet::new();
    let mut partners: Vec<CrossRefPartner> = globals
        .iter()
        .filter(|m| !(m.sheet == source.sheet && m.occurrence == source.occurrence))
        .filter(|m| seen.insert((m.sheet.clone(), m.occurrence.clone(), m.id)))
        .filter_map(|m| {
            let page = numbering.number_of(&m.sheet, &m.occurrence)?;
            (Some(page) != source_page).then(|| CrossRefPartner {
                sheet: m.sheet.clone(),
                occurrence: m.occurrence.clone(),
                label: m.id,
                page,
                at: m.at,
            })
        })
        .collect();
    partners.sort_by(|a, b| {
        a.page
            .cmp(&b.page)
            .then(a.at.x.total_cmp(&b.at.x))
            .then(a.at.y.total_cmp(&b.at.y))
            .then(a.label.cmp(&b.label))
    });
    partners
}
