//! Rule: DiffPairPartnerMissing (#2951).
//!
//! A differential pair needs both nets. The halves come from
//! `signex_net::lone_pair_members`: the present half of a pair a directive
//! marks, and a net named as a positive half (`USB_D_P`, `USB_DP`, `CLK+`)
//! whose negative partner is not on the sheet. One finding per net, at the
//! label that names it (so a Blanket or No-ERC there silences it, and the
//! finding is waivable by that label), else at the directive.

use signex_types::schematic::SelectedKind;

use crate::context::ErcContext;
use crate::diagnostic::Diagnostic;
use crate::{RuleKind, sel};

pub(crate) fn diff_pair_partner_missing(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    for lone in &ctx.lone_pairs {
        let message = match lone.directive {
            Some(_) => format!(
                "Net {} is in differential pair {} by a directive, but its partner {} is not on \
                 this sheet",
                lone.net, lone.pair, lone.partner
            ),
            None => format!(
                "Net {} is named as the positive half of differential pair {}, but its partner \
                 {} is not on this sheet",
                lone.net, lone.pair, lone.partner
            ),
        };
        let diagnostic = match (lone.label, lone.directive) {
            (Some((label, at)), _) => {
                Diagnostic::new(RuleKind::DiffPairPartnerMissing, message, at)
                    .with_primary(sel(label, SelectedKind::Label))
            }
            (None, Some((directive, at))) => {
                Diagnostic::new(RuleKind::DiffPairPartnerMissing, message, at)
                    .with_primary(sel(directive, SelectedKind::Directive))
            }
            (None, None) => continue,
        };
        out.push(diagnostic.tagged(format!("pair:{}", lone.pair)));
    }
}

#[cfg(test)]
#[path = "tests/diff_pair_partner_tests.rs"]
mod tests;
