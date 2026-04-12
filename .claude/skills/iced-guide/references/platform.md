# Iced Platform — SVG, Render Backends, WASM, Dialogs, Comet

## SVG

### Optimized SVG loading with LazyLock

Create the handle once, clone it in view:

```rust
use iced::widget::svg::{Handle, Svg};
use std::sync::LazyLock;

// Handle created once at startup
pub static ARROW_LEFT: LazyLock<Handle> =
    LazyLock::new(|| Handle::from_memory(include_bytes!("arrow-left.svg")));

// Call in view — clones the handle (cheap)
pub fn arrow_left<'a>() -> Svg<'a> {
    Svg::new(ARROW_LEFT.clone()).width(iced::Shrink)
}
```

### SVG in view

```rust
fn view(&self) -> Element<'_, Message> {
    column![
        arrow_left(),
        Svg::new(Handle::from_path("icon.svg"))
            .width(24)
            .height(24),
    ].into()
}
```

---

## Render Backends

### Default behavior
Iced tries **wgpu** first, falls back to **tiny-skia** (software renderer).

### Force software rendering

Environment variable (temporary):
```bash
ICED_BACKEND=tiny-skia cargo run
```

Cargo.toml (permanent):
```toml
[dependencies]
iced = { version = "0.14", default-features = false, features = [
    "tiny-skia",
    "x11",      # Linux X11
    "wayland",  # Linux Wayland
    "smol",     # async runtime
    "image"     # image support
] }
```

### Select specific GPU backend

```toml
# .cargo/config.toml
[env]
WGPU_BACKEND = "vulkan"  # Options: dx12 / vulkan / metal / gl
```

| Backend | Platform |
|---------|----------|
| `dx12` | Windows |
| `vulkan` | Windows, Linux |
| `metal` | macOS |
| `gl` | All (OpenGL fallback) |

---

## Native File Dialogs (rfd)

### Dependencies

```toml
[dependencies]
rfd = "0.15"
```

### Open file

```rust
fn open_file() -> Task<Message> {
    Task::future(
        rfd::AsyncFileDialog::new()
            .add_filter("Image Formats", &["png", "jpg", "jpeg"])
            .pick_file(),
    )
    .then(|handle| match handle {
        Some(file) => Task::perform(
            async move { load_file(file).await },
            Message::FileLoaded,
        ),
        None => Task::done(Message::FileCancelled),
    })
}
```

### Save file

```rust
fn save_file(content: Vec<u8>) -> Task<Message> {
    Task::future(
        rfd::AsyncFileDialog::new()
            .set_file_name("output.txt")
            .save_file(),
    )
    .then(move |handle| match handle {
        Some(file) => Task::perform(
            async move {
                file.write(&content).await
                    .map(|_| file.path().to_path_buf())
                    .map_err(|e| e.to_string())
            },
            Message::FileSaved,
        ),
        None => Task::done(Message::SaveCancelled),
    })
}
```

### Pick folder

```rust
rfd::AsyncFileDialog::new().pick_folder()
```

### Multiple files

```rust
rfd::AsyncFileDialog::new()
    .add_filter("Schematics", &["kicad_sch", "snxsch"])
    .pick_files()  // returns Option<Vec<FileHandle>>
```

---

## WASM (Web Assembly)

### Setup

1. Add target:
```bash
rustup target add wasm32-unknown-unknown
```

2. Install Trunk:
```bash
cargo install --locked trunk
```

3. Create `index.html`:
```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>My Iced App</title>
    <base data-trunk-public-url />
</head>
<body style="margin: 0; padding: 0; width:100%; height:100svh">
    <link data-trunk rel="rust" href="Cargo.toml"
          data-wasm-opt="z" data-bin="my-app" />
</body>
</html>
```

### Important features

```toml
[dependencies]
iced = { version = "0.14", features = [
    "web-colors",  # Fix color rendering (enabled by default)
    "webgl",       # Fallback for browsers without wgpu support
] }
```

### Running

```bash
trunk serve        # Dev server with hot reload
trunk build        # Production build → dist/
```

### Styled text on WASM

By default, only `Fira Sans Regular` is loaded. For bold/italic:

```rust
// Load additional fonts (only needed on WASM)
iced::application(...)
    .font(include_bytes!("FiraSans-Bold.ttf"))
    .run()
```

### Debugging WASM panics

```toml
[dependencies]
console_error_panic_hook = "0.1"
```

```rust
fn main() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    // ... iced::run(...)
}
```

---

## Comet (Debugging Tool)

Companion tool for iced providing:
- **Debugging capabilities** — inspect widget tree
- **Performance metrics** — frame times, layout passes
- **Custom metrics** — user-defined measurements
- **Time travel** — replay state changes (enabled by ELM architecture)

> Still WIP. See: https://github.com/iced-rs/iced/pull/2910

---

## Multi-Window

Iced supports multiple windows via `window::open()` Task.
Each window can have its own view.
Useful for:
- Detachable panels
- Custom dialogs
- Tool palettes

```rust
fn update(&mut self, message: Message) -> Task<Message> {
    match message {
        Message::OpenNewWindow => {
            let (id, task) = window::open(window::Settings::default());
            self.windows.insert(id);
            task.discard()
        }
        Message::CloseWindow(id) => {
            self.windows.remove(&id);
            window::close(id)
        }
    }
}
```
