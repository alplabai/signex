//! The status bar as clusters: cursor · grid and snap · tool and hint at the
//! left, separated by faint rules; the selection (on a board, the board
//! counters), the board angle, the zoom and the Panels button at the right.
//! [`segments`] says what the bar holds in which order; [`view`] draws it.

use iced::widget::{Row, Widget, container, mouse_area, row, space, svg};
use iced::{Element, Length, Theme};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::{button, text};

use super::Readout;
use super::graphite::{
    BAR_GAP, BAR_PADDING, CLUSTER_GAP, PANELS_ICON, SP, TS, UNIT_COLUMN, X_COLUMN, Y_COLUMN,
    cursor_texts, footprint_grid_readout, grid_text, layer_label, layer_pill, mono_button,
    mono_message, mono_text, no_cursor_texts, panels_button, rule, selection_pill, snap_label,
    snap_pill, view_angle, zoom_text,
};
use crate::app::StatusBarRequest;
use crate::styles::tokens::resolve;
use crate::styles::{ChromeMetrics, ti};

/// Theme the Panels icon is fetched for: it is tinted with the ink token,
/// which hides the theme's accent swap, so any theme gives the same.
const GLYPH_THEME: ThemeId = ThemeId::GraphiteDark;

/// One segment of the bar, in the order it is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Segment {
    /// X, Y and the unit after them.
    Cursor,
    /// A faint rule between two clusters.
    Rule,
    /// The Grid read-out and the Snap pill.
    GridSnap,
    /// The board's active layer.
    LayerPill,
    /// The tool's name and the live hint after it. A hint takes the free
    /// room and ends in an ellipsis rather than push the right side out.
    Tool,
    /// "Saving N commits…" while git commits are pending.
    Saving,
    /// The running import, a click cancels it.
    Import,
    /// The free room between the left and the right side, when no hint
    /// takes it.
    Spacer,
    /// The selection summary, a click opens Properties.
    Selection,
    /// A board's Routed, Unrouted, DRC and Selected counters (#2460); on a
    /// board they stand in for the selection summary.
    BoardCounters,
    /// The board view's angle.
    ViewAngle,
    /// A skipped library is still missing; a click reopens its dialog.
    LibraryMissing,
    /// "N missing assets": image files the sheet's project cannot show.
    AssetsMissing,
    Zoom,
    Panels,
}

/// The segments `r` shows, left to right.
pub(super) fn segments(r: &Readout<'_>) -> Vec<Segment> {
    use Segment as S;
    if r.surface.no_document {
        return no_document_segments(r);
    }
    let mut out = vec![S::Cursor, S::Rule, S::GridSnap, S::Rule];
    if r.surface.active_layer.is_some() {
        out.push(S::LayerPill);
    }
    out.push(S::Tool);
    if r.pending_git_commits > 0 {
        out.push(S::Saving);
    }
    if r.board_import.is_some() {
        out.push(S::Import);
    }
    if r.activity.is_none() {
        out.push(S::Spacer);
    }
    let before_zoom = out.len();
    if r.surface.board.is_some() {
        out.push(S::BoardCounters);
    } else if r.selection.is_some() {
        out.push(S::Selection);
    }
    if r.surface.view_angle.is_some() {
        out.push(S::ViewAngle);
    }
    if r.surface.missing_libraries > 0 {
        out.push(S::LibraryMissing);
    }
    if r.surface.missing_assets > 0 {
        out.push(S::AssetsMissing);
    }
    if out.len() > before_zoom {
        out.push(S::Rule);
    }
    out.extend([S::Zoom, S::Panels]);
    out
}

/// The bar with no document in view (mockup row 2): the empty cursor columns
/// and Panels. Work still running keeps its segment, so a save stays visible
/// and an import can still be cancelled.
fn no_document_segments(r: &Readout<'_>) -> Vec<Segment> {
    use Segment as S;
    let mut out = vec![S::Cursor];
    if r.pending_git_commits > 0 {
        out.push(S::Saving);
    }
    if r.board_import.is_some() {
        out.push(S::Import);
    }
    out.push(S::Spacer);
    if r.surface.missing_libraries > 0 {
        out.push(S::LibraryMissing);
    }
    out.push(S::Panels);
    out
}

/// Where the right side of `body` (the bar without Panels) starts.
fn right_side_start(body: &[Segment]) -> usize {
    body.iter()
        .position(|segment| {
            matches!(
                segment,
                Segment::Selection | Segment::BoardCounters | Segment::ViewAngle | Segment::Zoom
            )
        })
        .unwrap_or(body.len())
}

/// `part` of the bar as one row.
fn segment_row<'a>(
    part: &[Segment],
    r: &Readout<'_>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Row<Element<'a, StatusBarRequest>> {
    part.iter()
        .filter_map(|&segment| segment_view(segment, r, tokens, i18n))
        .fold(Row::new(), |bar, element| bar.push(element))
        .spacing(BAR_GAP)
        .align_y(iced::Alignment::Center)
}

/// The Graphite status bar: the [`segments`] of `r`. Panels is laid out
/// first and the right side next, so a narrow window clips the left side
/// and never pushes Panels past the edge (mockup row 6).
pub(super) fn view<'a>(
    r: &Readout<'_>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, StatusBarRequest> {
    let shown = segments(r);
    let (body, panels) = shown.split_at(shown.len().saturating_sub(1));
    let (left, right) = body.split_at(right_side_start(body));
    let mut main: Row<Element<'a, StatusBarRequest>> = Row::new()
        .spacing(BAR_GAP)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill)
        .clip(true)
        .push(
            segment_row(left, r, tokens, i18n)
                .width(Length::Fill)
                .clip(true)
                .boxed(),
        );
    if !right.is_empty() {
        main = main.push(segment_row(right, r, tokens, i18n).boxed());
    }
    let bar: Row<Element<'a, StatusBarRequest>> = Row::new()
        .spacing(BAR_GAP)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill)
        .push(main.boxed())
        .push(segment_row(panels, r, tokens, i18n).boxed());
    container(bar)
        .width(Length::Fill)
        .height(ChromeMetrics::DEFAULT.status)
        .padding(BAR_PADDING)
        .align_y(iced::alignment::Vertical::Center)
        .style(crate::styles::status_bar(tokens))
        .boxed()
}

/// The widgets of `segment`; `None` when `r` has nothing for it.
fn segment_view<'a>(
    segment: Segment,
    r: &Readout<'_>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Option<Element<'a, StatusBarRequest>> {
    let ink = ti(tokens.text);
    let muted = resolve(tokens).ink3;
    Some(match segment {
        Segment::Cursor => cursor_cluster(r, tokens, i18n),
        Segment::Rule => rule(tokens),
        Segment::GridSnap => grid_snap_cluster(r, tokens, i18n),
        Segment::LayerPill => layer_pill_view(r, tokens)?,
        Segment::Tool => tool_cluster(r, tokens, i18n),
        Segment::Saving => {
            let label = t!(i18n, ids::PANELS_STATUS_BAR_SAVING, "count" => r.pending_git_commits);
            mono_message(label, resolve(tokens).info).boxed()
        }
        Segment::Import => mono_button(
            mono_message(
                t!(i18n, ids::PANELS_STATUS_BAR_IMPORTING, "name" => r.board_import?),
                ink,
            ),
            StatusBarRequest::CancelBoardImport,
        )
        .boxed(),
        Segment::Spacer => space::horizontal().boxed(),
        Segment::Selection => button(mono_message(r.selection.clone()?, ink))
            .padding([1.5, 6.0])
            .style(selection_pill(tokens))
            .on_press(StatusBarRequest::OpenPropertiesForSelection)
            .boxed(),
        Segment::BoardCounters => {
            super::board::board_counters(r.surface.board.as_ref()?, tokens, i18n)
        }
        Segment::ViewAngle => view_angle(r.surface.view_angle.clone()?, tokens),
        Segment::LibraryMissing => button(mono_message(
            t!(i18n, ids::LIBRARY_RECOVERY_MISSING_CHIP, "count" => r.surface.missing_libraries),
            signex_widgets::theme_ext::warning_color(tokens),
        ))
        .padding([1.5, 6.0])
        .style(selection_pill(tokens))
        .on_press(StatusBarRequest::LibraryMissingChipClicked)
        .boxed(),
        Segment::AssetsMissing => container(mono_message(
            t!(i18n, ids::IMAGES_STATUS_MISSING, "count" => r.surface.missing_assets),
            signex_widgets::theme_ext::warning_color(tokens),
        ))
        .padding([1.5, 6.0])
        .boxed(),
        Segment::Zoom => mono_message(zoom_text(i18n, r.zoom), muted).boxed(),
        Segment::Panels => panels(tokens, i18n),
    })
}

/// X and Y in fixed, right-aligned columns, then the unit once; a click on
/// the unit cycles it.
fn cursor_cluster<'a>(
    r: &Readout<'_>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, StatusBarRequest> {
    let muted = resolve(tokens).ink3;
    let (x, y) = if r.surface.no_document {
        no_cursor_texts(i18n)
    } else {
        cursor_texts(i18n, r.unit, r.x, r.y)
    };
    row![
        mono_message(x, muted)
            .width(X_COLUMN)
            .align_x(iced::alignment::Horizontal::Right),
        mono_message(y, muted)
            .width(Y_COLUMN)
            .align_x(iced::alignment::Horizontal::Right),
        button(mono_text(r.unit.to_string(), muted))
            .padding([1.0, SP.xs])
            .width(UNIT_COLUMN)
            .style(button::text)
            .on_press(StatusBarRequest::CycleUnit),
    ]
    .spacing(CLUSTER_GAP)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// The Grid read-out (a click shows or hides the grid; a right-click opens
/// the Grid & Snap popover on a sheet, the Grid popover on a board, #2460;
/// in a footprint editor its caret opens the step list, mockup row 63) and
/// the Snap pill.
fn grid_snap_cluster<'a>(
    r: &Readout<'_>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, StatusBarRequest> {
    let muted = resolve(tokens).ink3;
    let label = mono_message(
        grid_text(i18n, r.grid_visible, r.grid_size_mm, r.unit),
        muted,
    );
    let grid: Element<'a, StatusBarRequest> = if r.surface.opens_grid_picker {
        footprint_grid_readout(label, tokens)
    } else if let Some(board) = &r.surface.board {
        let value = super::board::board_grid_value(
            i18n,
            r.grid_visible,
            r.grid_size_mm,
            r.unit,
            board.grid_extra,
        );
        let open = board.open == Some(super::BoardPopoverKind::Grid);
        super::board::board_grid_readout(value, open, tokens, i18n)
    } else if r.surface.opens_grid_popover {
        mouse_area(mono_button(label, StatusBarRequest::ToggleGrid))
            .on_right_press(StatusBarRequest::GridReadoutRightClicked)
            .boxed()
    } else {
        mono_button(label, StatusBarRequest::ToggleGrid).boxed()
    };
    let snap = button(mono_message(
        t!(i18n, snap_label(r.snap_enabled)),
        if r.snap_enabled {
            ti(tokens.success)
        } else {
            muted
        },
    ))
    .padding([1.5, 6.0])
    .style(snap_pill(tokens, r.snap_enabled))
    .on_press(StatusBarRequest::ToggleSnap);
    row![grid, snap]
        .spacing(CLUSTER_GAP)
        .align_y(iced::Alignment::Center)
        .boxed()
}

/// The active layer of an open board: its swatch, the caption and the name.
/// Under the Dropdown layer switcher it is a button with a chevron that
/// opens the list; under Strip it is a plain label. A right-click opens the
/// Active layer popover (#2460).
fn layer_pill_view<'a>(
    r: &Readout<'_>,
    tokens: &ThemeTokens,
) -> Option<Element<'a, StatusBarRequest>> {
    let layer = r.surface.active_layer.clone()?;
    let r_tokens = resolve(tokens);
    // The filled pill names the layer in its accent ink; the plain label in
    // the text ink (mockup row 6).
    let name_ink = if layer.opens_list {
        r_tokens.accent_soft_ink
    } else {
        ti(tokens.text)
    };
    let mut face = row![
        super::board::layer_swatch(layer.colour, tokens),
        text(layer.caption).size(TS.body).color(r_tokens.ink3),
        text(layer.name).size(TS.body).color(name_ink),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    let opened = |counter| StatusBarRequest::BoardCounterClicked {
        window: None,
        counter,
    };
    let pill: Element<'a, StatusBarRequest> = if layer.opens_list {
        face = face.push(super::board::layer_chevron(tokens));
        button(face)
            .padding([1.5, 6.0])
            .style(layer_pill(tokens))
            .on_press(opened(super::BoardCounter::Layer))
            .boxed()
    } else {
        container(face)
            .padding([1.5, 6.0])
            .style(layer_label(tokens))
            .boxed()
    };
    Some(
        mouse_area(pill)
            .on_right_press(opened(super::BoardCounter::ActiveLayer))
            .boxed(),
    )
}

/// The tool alone, in ink and the UI face, then the live hint in muted mono
/// (an editor error in the error colour). The hint fills the free room and
/// ends in an ellipsis when it is too long.
fn tool_cluster<'a>(
    r: &Readout<'_>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, StatusBarRequest> {
    let tool = text(t!(i18n, ids::PANELS_STATUS_BAR_TOOL, "tool" => r.tool.as_str()))
        .size(TS.body)
        .color(ti(tokens.text));
    let Some(activity) = r.activity else {
        return tool.boxed();
    };
    let ink = if activity.is_error {
        signex_widgets::theme_ext::error_color(tokens)
    } else {
        resolve(tokens).ink3
    };
    let hint = mono_text(activity.text.clone(), ink)
        .wrapping(iced::widget::text::Wrapping::None)
        .ellipsis(iced::widget::text::Ellipsis::End)
        .width(Length::Fill);
    row![tool, hint]
        .spacing(CLUSTER_GAP)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill)
        .boxed()
}

/// The bordered Panels button with the panels icon.
fn panels<'a>(tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, StatusBarRequest> {
    let ink = ti(tokens.text);
    let icon = svg(crate::icons::icon_chrome_panels(GLYPH_THEME))
        .width(PANELS_ICON)
        .height(PANELS_ICON)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        });
    button(
        row![
            icon,
            text(t!(i18n, ids::PANELS_STATUS_BAR_PANELS))
                .size(TS.section)
                .color(ink),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center),
    )
    .padding([1.5, 10.0])
    .style(panels_button(tokens))
    .on_press(StatusBarRequest::TogglePanelList)
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::super::{Activity, BoardReadout, LayerReadout, SurfaceReadout, ViewAngleReadout};
    use super::Segment as S;
    use super::*;
    use signex_types::coord::Unit;

    fn readout(activity: Option<&Activity>, surface: SurfaceReadout) -> Readout<'_> {
        Readout {
            x: 127.0,
            y: 45.72,
            grid_visible: true,
            snap_enabled: true,
            zoom: 100.0,
            unit: Unit::Mm,
            tool: "Wire".into(),
            grid_size_mm: 1.27,
            selection: None,
            pending_git_commits: 0,
            board_import: None,
            activity,
            surface,
        }
    }

    /// The mock's schematic bar: cursor · grid and snap · tool and hint
    /// (the hint takes the free room, so no spacer), then the selection
    /// pill, a rule, the zoom and Panels.
    #[test]
    fn a_schematic_bar_reads_in_three_clusters() {
        let i18n = I18n::en_us();
        let hint = Activity::note("Dragging with wires (Alt: plain move)".into());
        let mut r = readout(Some(&hint), SurfaceReadout::default());
        r.selection = Some(t!(
            i18n,
            ids::PANELS_STATUS_BAR_SELECTION_KINDS,
            "count" => 3,
            "kinds" => "2 components, 1 wire"
        ));
        assert_eq!(
            segments(&r),
            [
                S::Cursor,
                S::Rule,
                S::GridSnap,
                S::Rule,
                S::Tool,
                S::Selection,
                S::Rule,
                S::Zoom,
                S::Panels,
            ]
        );
    }

    /// A board's surface: the layer pill, the angle and the counters.
    fn board_surface(i18n: &I18n) -> SurfaceReadout {
        SurfaceReadout {
            active_layer: Some(LayerReadout {
                caption: t!(i18n, ids::PCB_STATUS_LAYER),
                name: t!(i18n, ids::PCB_STATUS_LAYER),
                colour: iced::Color::WHITE,
                opens_list: false,
            }),
            view_angle: Some(ViewAngleReadout {
                caption: t!(i18n, ids::PCB_STATUS_LAYER),
                value: "90°".into(),
                tooltip: t!(i18n, ids::PCB_STATUS_LAYER),
                placeholder: t!(i18n, ids::PCB_STATUS_LAYER),
                entry: None,
            }),
            opens_grid_popover: false,
            opens_grid_picker: false,
            board: Some(BoardReadout::default()),
            no_document: false,
            missing_libraries: 0,
            missing_assets: 0,
        }
    }

    /// The mock's board bar: the layer pill before the tool, the git save
    /// after it; at the right the board counters (#2460), then the angle
    /// before the rule.
    #[test]
    fn a_board_bar_shows_the_layer_pill_save_and_angle() {
        let i18n = I18n::en_us();
        let mut r = readout(None, board_surface(&i18n));
        r.pending_git_commits = 2;
        assert_eq!(
            segments(&r),
            [
                S::Cursor,
                S::Rule,
                S::GridSnap,
                S::Rule,
                S::LayerPill,
                S::Tool,
                S::Saving,
                S::Spacer,
                S::BoardCounters,
                S::ViewAngle,
                S::Rule,
                S::Zoom,
                S::Panels,
            ]
        );
    }

    /// Mockup row 164: on a board the Selected counter stands in for the selection
    /// summary; the counters come before the angle and the zoom.
    #[test]
    fn on_a_board_the_counters_replace_the_selection_summary() {
        let i18n = I18n::en_us();
        let mut r = readout(None, board_surface(&i18n));
        r.selection = Some(t!(
            i18n,
            ids::PANELS_STATUS_BAR_SELECTION_KINDS,
            "count" => 2,
            "kinds" => "2 components"
        ));
        let shown = segments(&r);
        assert!(shown.contains(&S::BoardCounters));
        assert!(!shown.contains(&S::Selection));
        let at = |s| shown.iter().position(|x| *x == s).expect("shown");
        assert!(at(S::BoardCounters) < at(S::ViewAngle));
        assert!(at(S::ViewAngle) < at(S::Zoom));
    }

    /// A sheet keeps its bar: no counters, the selection summary as before.
    #[test]
    fn a_sheet_bar_has_no_board_counters() {
        let i18n = I18n::en_us();
        let mut r = readout(None, SurfaceReadout::default());
        r.selection = Some(t!(
            i18n,
            ids::PANELS_STATUS_BAR_SELECTION_KINDS,
            "count" => 1,
            "kinds" => "1 wire"
        ));
        let shown = segments(&r);
        assert!(shown.contains(&S::Selection));
        assert!(!shown.contains(&S::BoardCounters));
        assert!(!shown.contains(&S::LayerPill));
    }

    /// With nothing at the right but the zoom, no rule stands before it;
    /// the unit is part of the cursor cluster, never beside the zoom.
    #[test]
    fn the_unit_is_never_at_the_right_and_a_lone_zoom_has_no_rule() {
        let r = readout(None, SurfaceReadout::default());
        let shown = segments(&r);
        assert_eq!(
            shown,
            [
                S::Cursor,
                S::Rule,
                S::GridSnap,
                S::Rule,
                S::Tool,
                S::Spacer,
                S::Zoom,
                S::Panels
            ]
        );
        assert_eq!(shown.first(), Some(&S::Cursor));
    }

    /// Mockup row 2: with no document the bar holds the cursor and Panels;
    /// grid, snap, tool and zoom describe a document, so they leave.
    #[test]
    fn with_no_document_the_bar_holds_the_cursor_and_panels() {
        let surface = SurfaceReadout {
            no_document: true,
            ..SurfaceReadout::default()
        };
        let r = readout(None, surface);
        assert_eq!(segments(&r), [S::Cursor, S::Spacer, S::Panels]);
        let (x, y) = no_cursor_texts(&I18n::en_us());
        assert!(x.into_string().contains('—'));
        assert!(y.into_string().contains('—'));
    }

    /// Mockup row 138: the chip shows with and without a document in view.
    #[test]
    fn a_skipped_missing_library_keeps_its_chip_on_the_bar() {
        for no_document in [false, true] {
            let surface = SurfaceReadout {
                no_document,
                missing_libraries: 1,
                ..SurfaceReadout::default()
            };
            assert!(segments(&readout(None, surface)).contains(&S::LibraryMissing));
        }
    }

    /// Mockup row 204: a sheet whose project misses image files counts them
    /// in a warning pill; with none missing the bar has no pill.
    #[test]
    fn missing_image_assets_are_counted_on_the_bar() {
        let i18n = I18n::en_us();
        let tokens = signex_types::theme::theme_tokens(ThemeId::GraphiteLight);
        let none = readout(None, SurfaceReadout::default());
        assert!(!segments(&none).contains(&S::AssetsMissing));
        let surface = SurfaceReadout {
            missing_assets: 2,
            ..SurfaceReadout::default()
        };
        let r = readout(None, surface);
        assert!(segments(&r).contains(&S::AssetsMissing));
        let texts = crate::layout_probe::lay_out_texts(
            view(&r, &tokens, &i18n),
            iced::Size::new(1440.0, 40.0),
        );
        assert!(
            texts.iter().any(|t| t
                .text
                .replace(['\u{2068}', '\u{2069}'], "")
                .contains("2 missing assets")),
            "{texts:?}"
        );
    }

    /// Running work stays on the no-document bar: an import keeps its cancel.
    #[test]
    fn with_no_document_running_work_keeps_its_segments() {
        let surface = SurfaceReadout {
            no_document: true,
            ..SurfaceReadout::default()
        };
        let mut r = readout(None, surface);
        r.pending_git_commits = 1;
        r.board_import = Some("board.zip");
        assert_eq!(
            segments(&r),
            [S::Cursor, S::Saving, S::Import, S::Spacer, S::Panels]
        );
    }

    /// Mockup row 6: however narrow the window, Panels stays inside the bar.
    #[test]
    fn the_panels_button_stays_inside_a_narrow_board_bar() {
        let i18n = I18n::en_us();
        let tokens = signex_types::theme::theme_tokens(ThemeId::GraphiteLight);
        let r = readout(None, board_surface(&i18n));
        for width in [1440.0_f32, 1000.0, 640.0] {
            let texts = crate::layout_probe::lay_out_texts(
                view(&r, &tokens, &i18n),
                iced::Size::new(width, 40.0),
            );
            let panels = texts
                .iter()
                .find(|t| t.text.contains("Panels"))
                .expect("Panels is drawn");
            assert!(
                panels.bounds.x + panels.bounds.width <= width,
                "{width}: {:?}",
                panels.bounds
            );
        }
    }

    /// The bar splits before the first right-side segment.
    #[test]
    fn the_right_side_starts_at_the_counters_or_the_zoom() {
        let i18n = I18n::en_us();
        let board = segments(&readout(None, board_surface(&i18n)));
        assert_eq!(board[right_side_start(&board)], S::BoardCounters);
        let sheet = segments(&readout(None, SurfaceReadout::default()));
        assert_eq!(sheet[right_side_start(&sheet)], S::Zoom);
    }

    /// The running import keeps its cancel segment.
    #[test]
    fn a_running_import_keeps_its_cancel_segment() {
        let mut r = readout(None, SurfaceReadout::default());
        r.board_import = Some("board.zip");
        assert!(segments(&r).contains(&S::Import));
    }

    /// Mockup rows 6 and 164: under Strip the active layer is a plain label
    /// with no fill; the Dropdown pill keeps its accent-soft fill.
    #[test]
    fn under_strip_the_active_layer_is_a_label_with_no_fill() {
        let i18n = I18n::en_us();
        let tokens = signex_types::theme::theme_tokens(ThemeId::GraphiteDark);
        let accent_soft = resolve(&tokens).accent_soft.into_rgba8();
        let window = iced::Size::new(600.0, 40.0);
        let has_fill = |opens_list: bool| {
            let mut surface = board_surface(&i18n);
            if let Some(layer) = surface.active_layer.as_mut() {
                layer.opens_list = opens_list;
            }
            let r = readout(None, surface);
            let pill = layer_pill_view(&r, &tokens).expect("a board has a layer pill");
            crate::layout_probe::draw_pixels(pill, window)
                .chunks_exact(4)
                .any(|p| (0..3).all(|i| p[i].abs_diff(accent_soft[i]) <= 1))
        };
        assert!(!has_fill(false), "the Strip label paints the accent fill");
        assert!(has_fill(true), "the Dropdown pill lost its accent fill");
    }
}
