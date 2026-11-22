//! Mockup group `pcb` (rows m93-m110).
//!
//! Every row is mapped. m107 (Length Matching) is not built. The
//! Properties rows m94-m102 open a board with one of every primitive
//! (`helpers_d::primitives_board`) and click the item.

use super::checks;
use super::helpers_d::{self, board_click, command};
use super::states;
use crate::fixture::{self, Fixture};
use crate::render::{self, Crop};
use crate::settle_support;
use crate::surface::{Check, Surface, WindowSel};
use signex_app::app::{
    LayerTabMenuMsg, LayerTabSubmenu, Message, PcbBarMenu, PcbBarMsg, PcbMsg, Signex,
};
use signex_app::panels::PanelKind;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};

pub fn rows() -> Vec<Surface> {
    vec![
        Surface {
            id: "m93",
            title: "Properties: Board Options",
            // The board open with nothing selected: Properties shows the board.
            open: |app, r, fx| {
                fixture::open_board(app, r, fx);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: BOARD,
        },
        Surface {
            id: "m94",
            title: "Properties: Component (PCB)",
            // One component; two selected is the mockup's other state.
            open: |app, r, fx| click_primitive(app, r, fx, (60.0, 10.0)),
            window: WindowSel::Auto,
            checks: COMPONENT,
        },
        Surface {
            id: "m95",
            title: "Properties: Track",
            open: |app, r, fx| click_primitive(app, r, fx, (5.0, 0.0)),
            window: WindowSel::Auto,
            checks: TRACK,
        },
        Surface {
            id: "m96",
            title: "Properties: Via",
            open: |app, r, fx| click_primitive(app, r, fx, (20.0, 0.0)),
            window: WindowSel::Auto,
            checks: VIA,
        },
        Surface {
            id: "m97",
            title: "Properties: Pad (PCB)",
            // The SMD pad; the through-hole pad at (58, 10) is the other state.
            open: |app, r, fx| click_primitive(app, r, fx, (62.0, 10.0)),
            window: WindowSel::Auto,
            checks: PAD,
        },
        Surface {
            id: "m98",
            title: "Properties: Arc",
            open: |app, r, fx| click_primitive(app, r, fx, (25.0, 20.0)),
            window: WindowSel::Auto,
            checks: ARC,
        },
        Surface {
            id: "m99",
            title: "Properties: Zone / pour",
            // A polygon pour; shelved and keepout are other states.
            open: |app, r, fx| click_primitive(app, r, fx, (55.0, 55.0)),
            window: WindowSel::Auto,
            checks: ZONE,
        },
        Surface {
            id: "m100",
            title: "Properties: Board text",
            open: |app, r, fx| click_primitive(app, r, fx, (5.0, 30.0)),
            window: WindowSel::Auto,
            checks: TEXT,
        },
        Surface {
            id: "m101",
            title: "Properties: Dimension",
            open: |app, r, fx| {
                let at = helpers_d::dimension_hit()?;
                click_primitive(app, r, fx, at)
            },
            window: WindowSel::Auto,
            checks: DIMENSION,
        },
        Surface {
            id: "m102",
            title: "Properties: Graphic / mask",
            // A silkscreen line; circle and text frame are other states.
            open: |app, r, fx| click_primitive(app, r, fx, (5.0, 40.0)),
            window: WindowSel::Auto,
            checks: GRAPHIC,
        },
        Surface {
            id: "m103",
            title: "Layers panel (+ view options)",
            // The row draws the View Options tab.
            open: states::open_view_options,
            window: WindowSel::Auto,
            checks: LAYERS,
        },
        Surface {
            id: "m104",
            title: "Layer tab context menu / layer strip",
            // The Top tab's right-click menu with its Hide flyout open. The
            // layer set menu (LayerSetMenuMsg::Opened) is the other state.
            open: |app, r, fx| {
                fixture::open_board(app, r, fx);
                // The menu opens at the pointer: over the strip under the canvas.
                app.interaction_state.last_mouse_pos = (420.0, 860.0);
                let tab_menu = |msg| Message::Pcb(PcbMsg::LayerTabMenu(msg));
                settle_support::settle(
                    app,
                    tab_menu(LayerTabMenuMsg::Opened {
                        window: None,
                        layer: PcbLayer::Copper(CopperLayer::Top),
                    }),
                );
                settle_support::settle(
                    app,
                    tab_menu(LayerTabMenuMsg::SubmenuOpened(LayerTabSubmenu::Hide)),
                );
                helpers_d::reached(
                    app.interaction_state.pcb_layer_tab_menu.is_some(),
                    "LayerTabMenuMsg::Opened left no layer tab menu",
                )
            },
            window: WindowSel::Auto,
            checks: LAYER_TAB_MENU,
        },
        Surface {
            id: "m105",
            title: "Layer Stack",
            // Today a dock panel; the mockup proposes a document tab.
            open: |app, r, fx| {
                fixture::open_board(app, r, fx);
                command(app, "board_layer_stack")?;
                fixture::select_panel(app, PanelKind::LayerStack)?;
                let _ = render::draw_main(r, app);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: LAYER_STACK,
        },
        Surface {
            id: "m106",
            title: "PCB (Nets, Net Classes, Differential Pairs, xSignals, Polygons)",
            // Today the PCB Nets panel the board seeds on the left; the one
            // panel with a mode dropdown is the mockup's proposal.
            open: |app, r, fx| {
                fixture::open_board(app, r, fx);
                fixture::select_panel(app, PanelKind::PcbNets)?;
                let _ = render::draw_main(r, app);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: PCB_NETS,
        },
        Surface {
            id: "m107",
            title: "Length Matching",
            open: |_, _, _| {
                Err(
                    "not built: no Length Matching tab or panel; only the paired \
                     header widget exists (panels/paired.rs) and no command opens it"
                        .into(),
                )
            },
            window: WindowSel::Auto,
            checks: &[],
        },
        Surface {
            id: "m108",
            title: "PCB active bar",
            // The bar with no dropdown open. The mockup opens Copper Areas, a
            // `PcbBarMenu::Group(PcbBarGroup::CopperAreas)`, and `PcbBarGroup`
            // is not exported.
            open: |app, r, fx| {
                fixture::open_board(app, r, fx);
                Ok(())
            },
            window: WindowSel::MainCrop(Crop {
                x: 300,
                y: 40,
                w: 840,
                h: 160,
            }),
            checks: ACTIVE_BAR,
        },
        Surface {
            id: "m109",
            title: "Board 3D view",
            open: |app, r, fx| {
                fixture::open_board(app, r, fx);
                command(app, "view_board_3d")?;
                let _ = render::draw_main(r, app);
                helpers_d::reached(
                    app.interaction_state.board_3d.is_some(),
                    "view_board_3d left the board flat",
                )
            },
            window: WindowSel::Auto,
            checks: BOARD_3D,
        },
        Surface {
            id: "m110",
            title: "Board sketch view + sketch attr panel",
            // Superseded by m179 in the mockup; kept for comparison.
            open: |app, r, fx| {
                fixture::open_board(app, r, fx);
                command(app, "board_sketch")?;
                let _ = render::draw_main(r, app);
                helpers_d::reached(
                    app.interaction_state.board_sketch.is_some(),
                    "board_sketch did not enter sketch mode",
                )
            },
            window: WindowSel::Auto,
            checks: BOARD_SKETCH,
        },
    ]
}

/// Open the primitives board and click it at `at` (mm).
fn click_primitive(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
    at: (f64, f64),
) -> Result<(), String> {
    helpers_d::open_primitives(app, r, fx)?;
    board_click(app, r, at.0, at.1);
    Ok(())
}

const BOARD: &[Check] = &[("a board tab is active", checks::board_tab_active)];

const COMPONENT: &[Check] = &[("Properties shows the component", helpers_d::component_form)];

const TRACK: &[Check] = &[("Properties shows the track", helpers_d::track_form)];

const VIA: &[Check] = &[("Properties shows the via", helpers_d::via_form)];

const PAD: &[Check] = &[("Properties shows the pad", helpers_d::pad_form)];

const ARC: &[Check] = &[("Properties shows the arc", helpers_d::arc_form)];

const ZONE: &[Check] = &[("Properties shows the pour", helpers_d::zone_form)];

const TEXT: &[Check] = &[("Properties shows the string", helpers_d::text_form)];

const DIMENSION: &[Check] = &[("Properties shows the dimension", helpers_d::dimension_form)];

const GRAPHIC: &[Check] = &[("Properties shows the drawing", helpers_d::graphic_form)];

const LAYERS: &[Check] = &[("the Layers panel is docked", layers_shown)];

const LAYER_TAB_MENU: &[Check] = &[("Dismissed closes the menu", layer_tab_menu_dismisses)];

const LAYER_STACK: &[Check] = &[("the Layer Stack panel is docked", layer_stack_shown)];

const PCB_NETS: &[Check] = &[("the PCB Nets panel is docked", pcb_nets_shown)];

const ACTIVE_BAR: &[Check] = &[
    ("a board tab is active", checks::board_tab_active),
    ("a bar dropdown opens", bar_menu_opens),
];

const BOARD_3D: &[Check] = &[("3D toggles back to the flat board", board_3d_toggles)];

const BOARD_SKETCH: &[Check] = &[("a board tab is active", checks::board_tab_active)];

fn layers_shown(app: &mut Signex) -> Result<(), String> {
    helpers_d::panel_shown(app, PanelKind::Layers)
}

fn layer_stack_shown(app: &mut Signex) -> Result<(), String> {
    helpers_d::panel_shown(app, PanelKind::LayerStack)
}

fn pcb_nets_shown(app: &mut Signex) -> Result<(), String> {
    helpers_d::panel_shown(app, PanelKind::PcbNets)
}

fn layer_tab_menu_dismisses(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Pcb(PcbMsg::LayerTabMenu(
        LayerTabMenuMsg::Dismissed,
    )));
    if app.interaction_state.pcb_layer_tab_menu.is_some() {
        Err("Dismissed left the layer tab menu open".into())
    } else {
        Ok(())
    }
}

fn bar_menu_opens(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Pcb(PcbMsg::Bar(PcbBarMsg::MenuToggled(
        PcbBarMenu::Move,
    ))));
    match app.interaction_state.pcb_bar_menu {
        Some(PcbBarMenu::Move) => Ok(()),
        _ => Err("MenuToggled(Move) opened no Move dropdown".into()),
    }
}

fn board_3d_toggles(app: &mut Signex) -> Result<(), String> {
    command(app, "view_board_3d")?;
    if app.interaction_state.board_3d.is_some() {
        Err("a second view_board_3d left the board in 3D".into())
    } else {
        Ok(())
    }
}
