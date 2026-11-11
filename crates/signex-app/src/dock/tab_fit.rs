//! How a dock tab strip shares its width: every tab is drawn, none is cut
//! off at the strip edge.

/// Widths of tabs whose natural widths are `natural` in a strip `available`
/// px wide. When they fit each keeps its natural width. Otherwise the narrow
/// tabs keep theirs and the wider ones share what is left equally, so a short
/// label is never shortened to make room for a long one.
pub(super) fn fit_widths(natural: &[f32], available: f32) -> Vec<f32> {
    let total: f32 = natural.iter().sum();
    if total <= available || natural.is_empty() {
        return natural.to_vec();
    }
    let mut sorted = natural.to_vec();
    sorted.sort_by(f32::total_cmp);
    let mut remaining = available.max(0.0);
    let mut cap = f32::INFINITY;
    for (i, &w) in sorted.iter().enumerate() {
        let share = remaining / (sorted.len() - i) as f32;
        if w > share {
            cap = share;
            break;
        }
        remaining -= w;
    }
    natural.iter().map(|&w| w.min(cap)).collect()
}

#[cfg(test)]
mod tests {
    use super::fit_widths;

    #[test]
    fn tabs_that_fit_keep_their_natural_widths() {
        assert_eq!(
            fit_widths(&[60.0, 90.0, 80.0], 240.0),
            vec![60.0, 90.0, 80.0]
        );
    }

    /// The footprint editor's left dock: four tabs in a 240 px dock used to
    /// clip the fourth to its first letter.
    #[test]
    fn an_overfull_strip_shrinks_the_wide_tabs_and_shows_every_tab() {
        let widths = fit_widths(&[70.0, 98.0, 86.0, 60.0], 240.0);
        let sum: f32 = widths.iter().sum();
        assert!((sum - 240.0).abs() < 0.01, "{widths:?}");
        assert_eq!(widths[3], 60.0, "the narrow tab keeps its width");
        assert!(widths.iter().all(|&w| w > 0.0));
        assert!(
            (widths[1] - widths[2]).abs() < 0.01,
            "wide tabs share equally"
        );
        assert!(widths[0] <= 70.0);
    }

    #[test]
    fn no_room_gives_zero_widths_without_panicking() {
        assert_eq!(fit_widths(&[50.0, 40.0], 0.0), vec![0.0, 0.0]);
        assert!(fit_widths(&[], 100.0).is_empty());
    }
}
