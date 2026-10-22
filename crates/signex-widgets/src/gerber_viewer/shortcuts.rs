use super::*;

pub trait GerberShortcutResolver {
    fn resolve_gerber_shortcut(
        &self,
        key: &keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Option<GerberViewerMessage>;
}

pub(super) fn next_layer_index(active_layer: Option<usize>, layer_count: usize) -> Option<usize> {
    if layer_count == 0 {
        return None;
    }

    match active_layer {
        Some(index) if index + 1 < layer_count => Some(index + 1),
        None => Some(0),
        _ => None,
    }
}

pub(super) fn previous_layer_index(
    active_layer: Option<usize>,
    layer_count: usize,
) -> Option<usize> {
    if layer_count == 0 {
        return None;
    }

    match active_layer {
        Some(index) if index > 0 && index < layer_count => Some(index - 1),
        None => Some(layer_count - 1),
        _ => None,
    }
}

pub(super) fn gerber_shortcut_message(
    resolver: &dyn GerberShortcutResolver,
    key: &keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> Option<GerberViewerMessage> {
    resolver.resolve_gerber_shortcut(key, modifiers)
}
