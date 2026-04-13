# iced_aw — Additional Widgets

> Source: https://github.com/iced-rs/iced_aw (v0.13, iced 0.14 compatible)

```toml
[dependencies]
iced    = "0.14"
iced_aw = { version = "0.13", features = ["full"] }
# or cherry-pick:
iced_aw = { version = "0.13", features = ["tabs", "menu", "number_input", "card"] }
```

Version mapping: `iced 0.14` → `iced_aw 0.13`

---

## TabBar / Tabs — document tabs

```rust
use iced_aw::{tab_bar, TabLabel};

// Manual tab bar + content switch:
column![
    tab_bar::TabBar::new(Message::TabSelected)
        .push(Tab::Schematic, TabLabel::Text("Schematic".into()))
        .push(Tab::Board,     TabLabel::Text("PCB".into()))
        .push(Tab::Symbols,   TabLabel::Text("Symbols".into()))
        .set_active_tab(&self.active_tab),
    match self.active_tab {
        Tab::Schematic => schematic_view(self).into(),
        Tab::Board     => board_view(self).into(),
        Tab::Symbols   => symbol_editor_view(self).into(),
    }
]
```

For dirty-tab indication (●) prepend a marker to the label string before passing
to `TabLabel::Text`.

---

## Menu / MenuBar — dropdown menu bar

```rust
use iced_aw::menu::{Item, Menu, MenuBar};

fn view_menu_bar(state: &State) -> MenuBar<'_, Message> {
    MenuBar::new(vec![
        Item::with_menu(
            button("File"),
            Menu::new(vec![
                Item::new(button("Open...").on_press(Message::OpenFile)),
                Item::new(button("Save").on_press(Message::Save)),
                Item::new(button("Save As...").on_press(Message::SaveAs)),
                Item::new(iced_aw::quad::Quad::default()),  // separator
                Item::new(button("Exit").on_press(Message::Exit)),
            ]).max_width(180.0),
        ),
        Item::with_menu(
            button("Edit"),
            Menu::new(vec![
                Item::new(button("Undo").on_press(Message::Undo)),
                Item::new(button("Redo").on_press(Message::Redo)),
                Item::new(iced_aw::quad::Quad::default()),
                Item::new(button("Select All").on_press(Message::SelectAll)),
                Item::new(button("Copy").on_press(Message::Copy)),
                Item::new(button("Paste").on_press(Message::Paste)),
            ]).max_width(180.0),
        ),
        Item::with_menu(
            button("View"),
            Menu::new(vec![
                Item::new(button("Fit All").on_press(Message::FitAll)),
                Item::new(button("Zoom In").on_press(Message::ZoomIn)),
                Item::new(button("Zoom Out").on_press(Message::ZoomOut)),
            ]).max_width(180.0),
        ),
    ])
}
```

For 8-menu bars (File, Edit, View, Place, Tools, Simulate, Help, …) use a
`stack![]` so the menu overlay renders above the canvas.

---

## NumberInput — numeric value entry

```rust
use iced_aw::NumberInput;

// Track width in mm:
NumberInput::new(
    self.track_width_mm,    // f64
    0.001..=10.0,           // range
    Message::TrackWidthChanged,
)
.step(0.01)
.precision(3)
.width(120)
```

Useful for: coordinate input, net width, drill diameter, rotation angle.

---

## Card — information card

```rust
use iced_aw::Card;

Card::new(
    text("Selected: R1"),
    column![
        row![text("Value:"),     text(&self.value)].spacing(8),
        row![text("Footprint:"), text(&self.footprint)].spacing(8),
        row![text("Net:"),       text(&self.net)].spacing(8),
    ].spacing(4),
)
.on_close(Message::CloseCard)
.max_width(280.0)
```

---

## Color Picker — layer colour selection

```rust
use iced_aw::ColorPicker;

ColorPicker::new(
    self.picker_open,          // bool
    self.layer_color,          // Color
    button("Set Color").on_press(Message::OpenPicker),
    Message::CancelPicker,
    Message::ColorSelected,    // fn(Color) -> Message
)
```

---

## SelectionList — layer / net list

```rust
use iced_aw::SelectionList;

SelectionList::new_with(
    &self.layers,
    Message::LayerSelected,    // fn(usize, LayerName) -> Message
    14.0,                      // font size
    5.0,                       // item padding
    iced_aw::selection_list::StyleSheet::default(),
    self.active_layer.as_ref(),
    iced::Font::default(),
)
.height(Fill)
```

---

## ContextMenu — right-click menu

```rust
use iced_aw::ContextMenu;

ContextMenu::new(
    canvas_widget,
    || column![
        button("Copy").on_press(Message::Copy),
        button("Paste").on_press(Message::Paste),
        button("Delete").on_press(Message::DeleteSelected),
        button("Properties").on_press(Message::ShowProperties),
    ]
    .padding(4)
    .into()
)
```

---

## LabeledFrame — sectioned properties panel

```rust
use iced_aw::LabeledFrame;

LabeledFrame::new(
    "Connection",
    column![
        row![text("Net:"),   text(&self.net_name)].spacing(8),
        row![text("Width:"), number_input(self.width, ...)].spacing(8),
    ].spacing(6).padding(8),
)
```

---

## Sidebar — left navigation panel

```rust
use iced_aw::sidebar;

sidebar::Sidebar::new(&self.active_panel, Message::PanelSelected)
    .push(Panel::Layers,     sidebar::Item::new(layer_icon, "Layers"))
    .push(Panel::Components, sidebar::Item::new(comp_icon,  "Components"))
    .push(Panel::Properties, sidebar::Item::new(prop_icon,  "Properties"))
    .push(Panel::Search,     sidebar::Item::new(search_icon,"Search"))
```

---

## Badge — label / count indicator

```rust
use iced_aw::Badge;

// DRC error count badge:
Badge::new(text(format!("{}", self.drc_error_count)))
    .style(iced_aw::badge::BadgeStyles::Danger)
```

---

## Full widget list

| Widget | Feature flag | EDA use |
|--------|-------------|---------|
| `TabBar` / `Tabs` | `tab_bar` / `tabs` | Document tabs (schematic / PCB / symbols) |
| `Menu` / `MenuBar` | `menu` | File / Edit / View / Place menu bar |
| `NumberInput` | `number_input` | Coordinates, widths, angles |
| `Card` | `card` | Properties panel, DRC result cards |
| `ColorPicker` | `color_picker` | Layer colour selection |
| `DatePicker` | `date_picker` | Revision date fields |
| `SelectionList` | `selection_list` | Layer list, net list |
| `ContextMenu` | `context_menu` | Right-click on canvas |
| `DropDown` | `drop_down` | Custom dropdown |
| `SlideBar` | `slide_bar` | Alternative slider |
| `Sidebar` | `sidebar` | Left panel navigation |
| `LabeledFrame` | `labeled_frame` | Properties section groups |
| `Badge` | `badge` | Net class tag, error count |
| `Quad` | `quad` | Menu separator, thin divider line |

Use `features = ["full"]` to enable everything at once.
