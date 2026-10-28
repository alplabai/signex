//! #1351 - the custom net-colour channel fields keep their value while the
//! text is not a number.

use super::*;
use crate::app::contracts::Channel;

fn red_after(app: &mut Signex, text: &str) -> f32 {
    let _ = app.dispatch_net_color_message(NetColorMsg::CustomChannel(Channel::R, text.into()));
    app.ui_state.net_color_custom.draft.r
}

#[test]
fn an_unparsable_channel_keeps_the_previous_value() {
    let (mut app, _task) = Signex::new();
    assert_eq!(red_after(&mut app, "128"), 128.0 / 255.0);

    assert_eq!(red_after(&mut app, ""), 128.0 / 255.0);
    assert_eq!(red_after(&mut app, "abc"), 128.0 / 255.0);
    assert_eq!(red_after(&mut app, "-3"), 128.0 / 255.0);
}

#[test]
fn a_channel_above_255_clamps_and_a_valid_one_applies() {
    let (mut app, _task) = Signex::new();
    assert_eq!(red_after(&mut app, "99999999999"), 1.0);
    assert_eq!(red_after(&mut app, " 0 "), 0.0);
}
