//! Signex AI panel: the AI Design shell. The layout is real; every action that
//! needs the AI backend is a disabled control with a tooltip.

mod state;
mod view;

pub use state::{SignalMode, SignalMsg};
pub use view::view;

/// Wrap a disabled stub in the "Coming in AI Design" tooltip, the tooltip of
/// every Signex AI / Start control that has no backend yet.
pub fn coming_soon<'a, M: 'a>(
    content: iced::Element<'a, M>,
    tokens: &signex_types::theme::ThemeTokens,
    i18n: &signex_i18n::I18n,
    position: iced::widget::tooltip::Position,
) -> iced::Element<'a, M> {
    use crate::styles::{TypeScale, ti};
    use iced::Widget as _;
    use iced::widget::container;
    use signex_widgets::{text, tooltip};
    let hint = container(
        text(signex_i18n::t!(i18n, signex_i18n::ids::PANELS_COMING_SOON))
            .size(TypeScale::DEFAULT.caption)
            .color(ti(tokens.text)),
    )
    .padding([3, 8])
    .style(signex_widgets::active_bar_dropdown::tooltip_style(
        ti(tokens.hover),
        ti(tokens.border),
    ));
    tooltip(content, hint, position).gap(4).boxed()
}
