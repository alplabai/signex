# Styling — iced 0.14

---

## How styling works

Every widget's `.style()` method accepts a closure:

```rust
widget.style(|theme: &Theme, status| {
    // theme: current application theme
    // status: widget state (Active, Hovered, Pressed, Disabled, …)
    WidgetStyle { /* fields */ }
})
```

The closure receives a `Theme` reference and a per-widget `Status` enum.
Use `theme.extended_palette()` to read all semantic colour roles.

---

## Built-in themes (23+ total)

```rust
use iced::Theme;

iced::application(...).theme(|_| Theme::Dark)

// Available themes:
Theme::Light
Theme::Dark
Theme::Dracula
Theme::Nord
Theme::SolarizedLight
Theme::SolarizedDark
Theme::GruvboxLight
Theme::GruvboxDark
Theme::CatppuccinLatte
Theme::CatppuccinFrappe
Theme::CatppuccinMacchiato
Theme::CatppuccinMocha    // default in Signex
Theme::TokyoNight
Theme::TokyoNightStorm
Theme::TokyoNightLight
Theme::KanagawaWave
Theme::KanagawaLotus
Theme::KanagawaDragon
Theme::Moonfly
Theme::Nightfly
Theme::Oxocarbon
Theme::Ferra
// ... more
```

---

## Palette and ExtendedPalette

```rust
let p = theme.palette();
// p.primary    Color
// p.success    Color
// p.warning    Color
// p.danger     Color
// p.text       Color
// p.background Color

let ext = theme.extended_palette();
// Each role has three tones:
// ext.primary.base.color    — main colour
// ext.primary.strong.color  — darker variant
// ext.primary.weak.color    — lighter variant
// Same for: success, warning, danger, secondary, background
```

---

## Button styling

```rust
use iced::widget::button;
use iced::Theme;

// Built-in convenience functions:
button("Primary").style(button::primary)
button("Secondary").style(button::secondary)
button("Success").style(button::success)
button("Danger").style(button::danger)
button("Text").style(button::text)

// Custom:
button("Custom").style(|theme: &Theme, status| {
    let p = theme.extended_palette();
    match status {
        button::Status::Active => button::Style {
            background: Some(p.success.strong.color.into()),
            text_color: Color::WHITE,
            border:     Border::rounded(8),
            shadow:     Shadow::default(),
        },
        button::Status::Hovered => button::Style {
            background: Some(p.success.base.color.into()),
            ..button::primary(theme, status)
        },
        _ => button::primary(theme, status),
    }
})
```

`button::Status` values: `Active`, `Hovered`, `Pressed`, `Disabled`

---

## Container styling

```rust
use iced::widget::container;
use iced::{Color, Border, Shadow};

// Built-in:
container(w).style(container::bordered_box)
container(w).style(container::rounded_box)
container(w).style(container::transparent)
container(w).style(container::primary)
container(w).style(container::secondary)
container(w).style(container::background)

// Custom:
container(w).style(|theme: &Theme| container::Style {
    text_color:  Some(Color::WHITE),
    background:  Some(Color::from_rgb(0.1, 0.1, 0.3).into()),
    border: Border {
        color:  Color::from_rgb(0.5, 0.5, 0.9),
        width:  2.0,
        radius: 12.0.into(),
    },
    shadow: Shadow {
        color:       Color::from_rgba(0.0, 0.0, 0.0, 0.3),
        offset:      iced::Vector::new(2.0, 2.0),
        blur_radius: 8.0,
    },
})
```

---

## Text styling

```rust
use iced::widget::text;

text("...").style(text::primary)
text("...").style(text::secondary)
text("...").style(text::success)
text("...").style(text::warning)
text("...").style(text::danger)

text("...").style(|theme: &Theme| text::Style {
    color: Some(theme.palette().primary),
})
```

---

## Custom Theme

```rust
use iced::Theme;
use iced::theme::Palette;
use iced::Color;

let my_theme = Theme::custom(
    "VS Code Dark".to_string(),
    Palette {
        background: Color::from_rgb(0.12, 0.12, 0.12),
        text:       Color::WHITE,
        primary:    Color::from_rgb(0.0, 0.47, 0.83),
        success:    Color::from_rgb(0.23, 0.70, 0.44),
        warning:    Color::from_rgb(0.99, 0.74, 0.13),
        danger:     Color::from_rgb(0.96, 0.36, 0.36),
    },
);

iced::application(...).theme(move |_| my_theme.clone())
```

Six EDA themes used in practice: Catppuccin Mocha, VS Code Dark, Altium Dark,
GitHub Dark, Solarized Light, Nord. All are expressible through `Theme::custom`.

---

## Gradient background

```rust
use iced::gradient;

container(w).style(|_theme| container::Style {
    background: Some(
        gradient::Linear::new(45)
            .add_stop(0.0, Color::from_rgb(0.1, 0.1, 0.5))
            .add_stop(1.0, Color::from_rgb(0.5, 0.1, 0.1))
            .into()
    ),
    ..Default::default()
})
```

---

## Border and Shadow

```rust
use iced::{Border, Shadow, Color, Vector};

let border = Border {
    color:  Color::from_rgb(0.5, 0.5, 0.5),
    width:  1.0,
    radius: 8.0.into(),
};

// Per-corner radii [top-left, top-right, bottom-right, bottom-left]:
let border = Border {
    radius: [4.0, 12.0, 12.0, 4.0].into(),
    ..Default::default()
};

let shadow = Shadow {
    color:       Color::from_rgba(0.0, 0.0, 0.0, 0.4),
    offset:      Vector::new(0.0, 4.0),
    blur_radius: 12.0,
};
```

---

## themer widget (scoped theme override)

```rust
use iced::widget::themer;

// Apply a specific theme to all children without changing the app theme:
themer(Theme::Light, content_widget)
```

---

## Status reference

| Widget | Status variants |
|--------|----------------|
| `button` | Active, Hovered, Pressed, Disabled |
| `text_input` | Active, Hovered, Focused, Disabled |
| `slider` | Active, Hovered, Dragging |
| `checkbox` | Active, Hovered, Disabled |
| `toggler` | Active, Hovered, Disabled |
| `pick_list` | Active, Hovered, Opened, Disabled |
| `radio` | Active, Hovered, Disabled |
| `scrollable` | Active, Hovered, Dragged |
