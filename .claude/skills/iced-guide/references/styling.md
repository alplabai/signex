# Iced Themes and Styling

## Setting a Theme

### Static theme

```rust
fn main() -> iced::Result {
    iced::application(...)
        .theme(|_| iced::Theme::Dracula)
        .run()
}
```

### Dynamic theme from state

```rust
struct App {
    theme: Option<Theme>,
}

impl App {
    fn theme(&self) -> Option<Theme> {
        self.theme.clone()
    }
}

fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .theme(App::theme)
        .run()
}
```

**Returning `Option<Theme>`:** `None` falls back to OS dark/light preference.
This is reactive — changing the OS preference updates the app automatically.

### Theme picker

```rust
use iced::widget::pick_list;

fn view(&self) -> Element<'_, Message> {
    pick_list(Theme::ALL, self.theme.clone(), Message::ThemeChanged)
        .placeholder("Choose a theme...")
        .into()
}
```

---

## Built-in Themes

`Theme::ALL` contains all built-in themes. Common ones:
- `Theme::Light`, `Theme::Dark`
- `Theme::Dracula`, `Theme::Nord`
- `Theme::SolarizedLight`, `Theme::SolarizedDark`
- `Theme::TokyoNight`, `Theme::TokyoNightStorm`
- `Theme::CatppuccinMocha`, `Theme::CatppuccinLatte`
- `Theme::GruvboxDark`, `Theme::GruvboxLight`
- `Theme::Nightfly`, `Theme::Oxocarbon`

---

## Custom Themes

### Using Theme::custom

```rust
let custom = Theme::custom("My Theme".to_string(), Palette {
    background: Color::from_rgb(0.1, 0.1, 0.15),
    text: Color::WHITE,
    primary: Color::from_rgb(0.3, 0.5, 1.0),
    success: Color::from_rgb(0.3, 0.8, 0.3),
    danger: Color::from_rgb(1.0, 0.3, 0.3),
});
```

### Using Theme::custom_with_fn

For greater control over the generated extended palette.

### Fully custom theme type

Requirements:
1. Implement `Base` trait for your type
2. For each widget, implement its `Catalog` trait and dependencies

See `iced_material` for a reference implementation.

---

## Per-Widget Styling

### Built-in style functions

```rust
button("Danger").style(button::danger)
button("Secondary").style(button::secondary)
button("Text").style(button::text)
```

### Inline closure style

The closure receives `(&Theme, Status)` and returns `Style`:

```rust
button("Custom")
    .style(|_, _| button::Style {
        background: Some(color!(0x1e1e2e).into()),
        text_color: color!(0xc0ffee),
        border: border::rounded(10),
        ..Default::default()
    })
```

### Named style function

```rust
fn button_danger_text(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();

    let base = button::Style {
        text_color: palette.danger.base.color,
        ..button::Style::default()
    };

    match status {
        button::Status::Active | button::Status::Pressed => base,
        button::Status::Hovered => button::Style {
            text_color: palette.danger.base.color.scale_alpha(0.8),
            ..base
        },
        button::Status::Disabled => button::Style {
            text_color: palette.danger.base.color.scale_alpha(0.5),
            ..base
        }
    }
}

// Usage:
button("Delete").style(button_danger_text)
```

---

## Status-Aware Styling

Most widgets pass a `Status` enum to the style function:

### Button statuses
- `button::Status::Active` — default state
- `button::Status::Hovered` — mouse over
- `button::Status::Pressed` — being clicked
- `button::Status::Disabled` — `on_press` not set

### Conditional enable/disable

```rust
// on_press_maybe: None = disabled, Some = enabled
button("Start").on_press_maybe(
    if self.is_running {
        None
    } else {
        Some(Message::Start)
    }
)
```

---

## Extended Palette

Access theme colors programmatically:

```rust
let palette = theme.extended_palette();

// Primary colors
palette.primary.base.color    // main primary
palette.primary.base.text     // text on primary bg
palette.primary.weak.color    // lighter primary
palette.primary.strong.color  // darker primary

// Other pairs: secondary, success, danger
palette.danger.base.color
palette.success.base.color

// Background colors
palette.background.base.color
palette.background.weak.color
palette.background.strong.color
```

---

## Color Utilities

```rust
use iced::{Color, color};

// Hex macro
color!(0x1e1e2e)           // RGB from hex
color!(0x1e1e2e, 0.5)      // RGBA with alpha

// From RGB
Color::from_rgb(0.5, 0.5, 0.5)
Color::from_rgb8(128, 128, 128)    // 0-255 range

// Alpha manipulation
color.scale_alpha(0.5)     // multiply alpha by 0.5
```

---

## Container Styling

```rust
container(content)
    .style(|theme| container::Style {
        background: Some(theme.extended_palette().background.weak.color.into()),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    })
```

---

## Text Styling

```rust
text("Hello")
    .style(|_| text::Style {
        color: Some(Color::from_rgb(0.8, 0.2, 0.2)),
    })
    .size(16)
    .font(Font::MONOSPACE)
```
