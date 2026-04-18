# iced_aw 0.13 — Additional Widgets for Iced 0.14

> iced_aw versioning: 0.13 targets iced 0.14, 0.11/0.12 target iced 0.13

## Setup

```toml
[dependencies]
iced = "0.14"
iced_aw = { version = "0.13", default-features = false, features = [
    "tabs", "card", "context_menu", "number_input", "color_picker",
    "menu", "drop_down", "selection_list", "wrap", "spinner",
    "sidebar", "labeled_frame", "badge",
] }
```

All widgets are feature-gated. Use `features = ["full"]` for everything.

---

## MenuBar + Menu + Item

Hierarchical drop-down menu system with automatic overlay positioning.

```rust
use iced_aw::menu::{Item, Menu, MenuBar};
use iced_aw::style::menu_bar as menu_style;

let menu_template = |items| Menu::new(items).max_width(240.0).offset(4.0).spacing(2.0);

let file_menu = Item::with_menu(
    button("File").style(button::text),
    menu_template(vec![
        Item::new(button("New").on_press(Message::New)),       // leaf
        Item::new(container(horizontal_rule(1)).width(Fill)),  // separator
        Item::with_menu(button("Recent"), menu_template(vec![ // nested submenu
            Item::new(button("file1.txt").on_press(Message::OpenRecent("file1.txt"))),
        ])),
    ]),
);

let mb = MenuBar::new(vec![file_menu])
    .spacing(1.0)
    .padding([1, 4])
    .close_on_item_click_global(true)
    .close_on_background_click_global(true)
    .style(|_theme, _status| menu_style::Style {
        bar_background: Color::from_rgb(0.12, 0.12, 0.13).into(),
        menu_background: Color::from_rgb(0.12, 0.12, 0.14).into(),
        menu_border: Border { width: 1.0, radius: 4.0.into(), color: BORDER_COLOR },
        menu_shadow: Shadow { color: Color::from_rgba(0.0, 0.0, 0.0, 0.5), offset: Vector::new(2.0, 4.0), blur_radius: 8.0 },
        path: TAB_ACTIVE_BG.into(),
        ..Default::default()
    });
```

**MenuBar methods:** `.width()`, `.height()`, `.spacing()`, `.padding()`, `.safe_bounds_margin()`, `.draw_path()`, `.scroll_speed()`, `.close_on_item_click_global()`, `.close_on_background_click_global()`, `.style()`, `.class()`

**Menu methods:** `.max_width()`, `.width()`, `.spacing()`, `.offset()`, `.padding()`, `.close_on_item_click()`, `.close_on_background_click()`

**Item:** `Item::new(widget)` (leaf) or `Item::with_menu(widget, menu)` (submenu)

**Macros:** `menu_bar!()`, `menu_items!()`, `menu!()`

---

## NumberInput

Numeric text input with +/- step buttons. Requires `num-traits` crate.

```rust
use iced_aw::NumberInput;

NumberInput::new(&value, 0.0..=100.0, Message::ValueChanged)
    .step(0.5)
    .width(Length::Fill)
    .padding(4)
    .ignore_buttons(false)  // show +/- buttons
    .ignore_scroll(false)   // allow mouse scroll to change value
```

**T bounds:** `T: Num + NumAssignOps + PartialOrd + Display + FromStr + Clone + Bounded`

**Methods:** `.id()`, `.step()`, `.bounds()`, `.width()`, `.padding()`, `.set_size()`, `.font()`, `.icon()`, `.align_x()`, `.line_height()`, `.on_submit()`, `.on_paste()`, `.ignore_buttons()`, `.ignore_scroll()`, `.style()`, `.input_style()`, `.class()`

---

## ContextMenu

Right-click overlay on any widget. Handles positioning automatically.

```rust
use iced_aw::ContextMenu;

ContextMenu::new(
    text("Right-click me"),  // underlay
    || {                     // closure builds overlay content
        column![
            button("Cut").on_press(Message::Cut),
            button("Copy").on_press(Message::Copy),
            button("Paste").on_press(Message::Paste),
        ].into()
    },
)
.style(|theme, _status| iced_aw::context_menu::Style {
    background: theme.extended_palette().background.base.color.into(),
})
```

**Behavior:** Right-click toggles overlay. Click outside / Escape closes. Positions at cursor.

**Caveat for canvas:** ContextMenu intercepts right-click before child widgets. If the underlay needs right-click for other purposes (e.g., canvas pan), use manual Stack-based overlay instead.

---

## DropDown

Overlay attached to an anchor widget. Controlled via `expanded` bool.

```rust
use iced_aw::DropDown;

DropDown::new(
    button("Select...").on_press(Message::ToggleDropdown),  // underlay
    column![                                                 // overlay content
        button("Option A").on_press(Message::SelectA),
        button("Option B").on_press(Message::SelectB),
    ],
    self.dropdown_open,  // expanded: bool
)
.width(200)
.alignment(iced_aw::drop_down::Alignment::Bottom)
.offset(5.0)
.on_dismiss(Message::CloseDropdown)
```

**Alignment:** `Top`, `Bottom`, `Left`, `Right`, `TopStart`, `TopEnd`, `BottomStart`, `BottomEnd`, `LeftStart`, `LeftEnd`, `RightStart`, `RightEnd`

---

## SelectionList

Scrollable list with keyboard-navigable single selection.

```rust
use iced_aw::SelectionList;

SelectionList::new(&self.items, |index, item| Message::Selected(index, item))
    .width(Length::Fill)
    .height(300)
```

**T bounds:** `T: Clone + ToString + Eq + Hash`

**Callback:** `Fn(usize, T) -> Message` — receives index and cloned item.

---

## Tabs (TabBar + Content)

Managed tab container with built-in tab bar and content switching.

```rust
use iced_aw::{Tabs, TabLabel, TabBarPosition};

Tabs::new_with_tabs(
    vec![
        (TabId::General, TabLabel::Text("General".into()), general_content),
        (TabId::Params, TabLabel::Text("Parameters".into()), params_content),
    ],
    Message::TabSelected,
)
.set_active_tab(&self.active_tab)
.tab_bar_position(TabBarPosition::Top)
.on_close(Message::TabClosed)
```

**TabLabel:** `TabLabel::Text(String)`, `TabLabel::Icon(char)`, `TabLabel::IconText(char, String)`

**TabId:** Any `Eq + Clone` type (typically an enum).

---

## TabBar (standalone)

Tab bar without managed content — for cases where you handle content switching yourself.

```rust
use iced_aw::{TabBar, TabLabel};

TabBar::with_tab_labels(
    vec![
        (0, TabLabel::Text("Tab 1".into())),
        (1, TabLabel::Text("Tab 2".into())),
    ],
    Message::TabSelected,
)
.set_active_tab(&self.active_tab)
.on_close(Message::TabClosed)
.text_size(12.0)
```

---

## Card

Elevated container with header, body, and optional footer.

```rust
use iced_aw::Card;

Card::new(
    text("Card Title"),   // head
    text("Card content"), // body
)
.foot(row![button("OK"), button("Cancel")])
.on_close(Message::CloseCard)
.max_width(400.0)
.padding(10.0)
```

---

## ColorPicker

HSV color picker overlay triggered by a button.

```rust
use iced_aw::ColorPicker;

ColorPicker::new(
    self.show_picker,           // bool
    self.current_color,         // Color
    button("Pick Color").on_press(Message::TogglePicker),  // underlay
    Message::CancelPicker,      // on_cancel
    Message::ColorSubmitted,    // on_submit: Fn(Color) -> Message
)
.on_color_change(Message::ColorChanged)  // real-time updates
```

---

## Badge

Small status indicator overlay on any widget.

```rust
use iced_aw::Badge;

Badge::new(text("3"))
    .padding(4)
    .style(|_theme, _status| badge::Style {
        background: Color::from_rgb(1.0, 0.2, 0.2).into(),
        text_color: Color::WHITE,
        border: Border::default(),
    })
```

---

## Spinner

Animated loading indicator.

```rust
use iced_aw::Spinner;

Spinner::new()
    .width(20)
    .height(20)
    .circle_radius(2.0)
```

---

## Wrap

Wrapping row/column — items flow to next line when space runs out.

```rust
use iced_aw::Wrap;

// Horizontal wrap
Wrap::with_elements(items)
    .spacing(4.0)
    .line_spacing(4.0)
    .padding(8)

// Vertical wrap
Wrap::with_elements_vertical(items)
    .spacing(4.0)
```

---

## Sidebar

Vertical tab bar for side navigation (like VS Code activity bar).

```rust
use iced_aw::{Sidebar, TabLabel};

Sidebar::with_tab_labels(
    vec![
        (PanelId::Projects, TabLabel::IconText('\u{e8b7}', "Projects".into())),
        (PanelId::Components, TabLabel::IconText('\u{e1b1}', "Components".into())),
    ],
    Message::SidebarSelect,
)
.set_active_tab(&self.active_panel)
.width(48)
```

---

## LabeledFrame

Container with a border and title label that breaks the border line.

```rust
use iced_aw::widget::labeled_frame::LabeledFrame;

LabeledFrame::new(
    text("Section Title").size(12),
    column![/* content */],
)
.width(Length::Fill)
.stroke_width(1.0)
.inset(8.0)
```

---

## Common Style Pattern

All widgets use the Catalog trait pattern:

```rust
// Status enum (shared)
pub enum Status { Active, Hovered, Pressed, Focused, Disabled }

// Style function signature
.style(|theme: &Theme, status: Status| WidgetStyle { ... })

// Or use built-in styles
.style(widget::primary)
.style(widget::secondary)
```
