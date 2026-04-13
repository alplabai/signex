# Custom Widget and Multi-Screen Composition — iced 0.14

---

## Widget trait — implement your own widget

`iced::advanced::widget::Widget` has three required methods: `size`, `layout`, `draw`.

```rust
use iced::advanced::widget::{self, Widget};
use iced::advanced::{Layout, Renderer, Shell};
use iced::advanced::layout::{Limits, Node};
use iced::advanced::renderer;
use iced::{Color, Element, Length, Rectangle, Size};
use iced::mouse::Cursor;

struct Circle {
    radius: f32,
    color:  Color,
}

impl<Message, Theme, R> Widget<Message, Theme, R> for Circle
where
    R: Renderer,
{
    // 1. Declare the widget's desired width and height.
    fn size(&self) -> Size<Length> {
        let diameter = self.radius * 2.0;
        Size {
            width:  Length::Fixed(diameter),
            height: Length::Fixed(diameter),
        }
    }

    // 2. Compute the layout node within the given limits.
    fn layout(
        &self,
        _tree: &mut widget::Tree,
        _renderer: &R,
        limits: &Limits,
    ) -> Node {
        let diameter = self.radius * 2.0;
        Node::new(limits.resolve(
            Length::Fixed(diameter),
            Length::Fixed(diameter),
            Size::new(diameter, diameter),
        ))
    }

    // 3. Draw into the renderer.
    fn draw(
        &self,
        _tree: &widget::Tree,
        renderer: &mut R,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: Cursor,
        _viewport: &Rectangle,
    ) {
        renderer.fill_quad(
            renderer::Quad {
                bounds: layout.bounds(),
                border: iced::Border {
                    radius: self.radius.into(),
                    ..Default::default()
                },
                shadow: Default::default(),
            },
            self.color,
        );
    }
}

// Conversion to Element:
impl<'a, Message, Theme, R> From<Circle>
    for Element<'a, Message, Theme, R>
where
    R: Renderer,
{
    fn from(c: Circle) -> Self { Self::new(c) }
}
```

---

## Optional Widget methods

```rust
// Internal per-widget state stored in the widget tree:
fn tag(&self) -> widget::tree::Tag {
    widget::tree::Tag::of::<MyState>()
}
fn state(&self) -> widget::tree::State {
    widget::tree::State::new(MyState::default())
}

// Event handling:
fn on_event(
    &mut self,
    tree:      &mut widget::Tree,
    event:     iced::Event,
    layout:    Layout<'_>,
    cursor:    Cursor,
    _renderer: &R,
    _clipboard: &mut dyn iced::advanced::Clipboard,
    shell:     &mut Shell<'_, Message>,
    _viewport: &Rectangle,
) -> iced::event::Status {
    use iced::Event::Mouse;
    use iced::mouse::{Event::ButtonPressed, Button::Left};

    if let Mouse(ButtonPressed(Left)) = event {
        if cursor.is_over(layout.bounds()) {
            shell.publish(self.on_press.clone());
            return iced::event::Status::Captured;
        }
    }
    iced::event::Status::Ignored
}

// Mouse cursor appearance:
fn mouse_interaction(
    &self,
    _tree: &widget::Tree,
    layout: Layout<'_>,
    cursor: Cursor,
    _viewport: &Rectangle,
    _renderer: &R,
) -> iced::mouse::Interaction {
    if cursor.is_over(layout.bounds()) {
        iced::mouse::Interaction::Pointer
    } else {
        iced::mouse::Interaction::default()
    }
}

// Composite widgets — declare and diff children:
fn children(&self) -> Vec<widget::Tree> {
    vec![widget::Tree::new(&self.child)]
}
fn diff(&self, tree: &mut widget::Tree) {
    tree.diff_children(std::slice::from_ref(&self.child));
}
```

---

## Component — mini-TEA widget

For a widget that needs its own internal update/view cycle, implement `Component`.

```rust
use iced::widget::component::{self, Component};
use iced::Element;

struct NumericInput {
    value:     Option<u32>,
    on_change: Box<dyn Fn(Option<u32>) -> Message>,
}

#[derive(Debug, Clone)]
enum Event {
    TextChanged(String),
    Increment,
    Decrement,
}

#[derive(Default)]
struct State {
    raw: String,
}

impl<Message: Clone + 'static> Component<Message> for NumericInput {
    type State = State;
    type Event = Event;

    fn update(&self, state: &mut State, event: Event) -> Option<Message> {
        match event {
            Event::TextChanged(s) => {
                state.raw = s.clone();
                Some((self.on_change)(s.parse().ok()))
            }
            Event::Increment => {
                let v = self.value.unwrap_or(0).saturating_add(1);
                Some((self.on_change)(Some(v)))
            }
            Event::Decrement => {
                let v = self.value.unwrap_or(0).saturating_sub(1);
                Some((self.on_change)(Some(v)))
            }
        }
    }

    fn view(&self, state: &State) -> Element<'_, Event> {
        use iced::widget::{button, row, text_input};
        row![
            button("-").on_press(Event::Decrement),
            text_input("0", &state.raw)
                .on_input(Event::TextChanged)
                .width(80),
            button("+").on_press(Event::Increment),
        ]
        .spacing(4)
        .into()
    }
}

// Usage in view:
component(NumericInput {
    value:     state.track_width,
    on_change: Box::new(Message::TrackWidthChanged),
})
```

---

## Multi-screen composition

Split screens into separate structs and use an enum to manage transitions.
Use `.map()` to convert child messages to parent messages.

```rust
enum Screen {
    Splash(splash::State),
    Editor(editor::State),
    Settings(settings::State),
}

#[derive(Debug, Clone)]
enum Message {
    Splash(splash::Message),
    Editor(editor::Message),
    Settings(settings::Message),
}

fn update(&mut self, msg: Message) -> Task<Message> {
    match (&mut self.screen, msg) {
        (Screen::Splash(s), Message::Splash(m)) => {
            let task = s.update(m);
            if s.is_done() {
                self.screen = Screen::Editor(editor::State::new());
            }
            task.map(Message::Splash)
        }
        (Screen::Editor(s), Message::Editor(m))   => s.update(m).map(Message::Editor),
        (Screen::Settings(s), Message::Settings(m)) => s.update(m).map(Message::Settings),
        _ => Task::none(),
    }
}

fn view(&self) -> Element<'_, Message> {
    match &self.screen {
        Screen::Splash(s)   => s.view().map(Message::Splash),
        Screen::Editor(s)   => s.view().map(Message::Editor),
        Screen::Settings(s) => s.view().map(Message::Settings),
    }
}

fn subscription(&self) -> Subscription<Message> {
    match &self.screen {
        Screen::Editor(s)   => s.subscription().map(Message::Editor),
        _                   => Subscription::none(),
    }
}
```

---

## Daemon — windowless background application

```rust
pub fn main() -> iced::Result {
    iced::daemon("Background Service", MyDaemon::update, MyDaemon::view)
        .subscription(MyDaemon::subscription)
        .run()
}

impl MyDaemon {
    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::ShowWindow => {
                iced::window::open(iced::window::Settings::default())
                    .map(|(id, _)| Message::WindowOpened(id))
            }
            _ => Task::none(),
        }
    }

    fn view(&self, _window: iced::window::Id) -> Element<'_, Message> {
        iced::widget::text("Running in background").into()
    }
}
```

---

## Practical tips

**Keep Message enum shallow** — two levels is usually enough:
`Message::Editor(editor::Message)`, not deeper.

**Parallel tasks with `Task::batch`**:
```rust
Task::batch([load_schematic(path), load_libraries(), fetch_symbols()])
```

**Always `#[derive(Debug, Clone)]` on Message** — required; also makes debugging trivial.

**Avoid `into()` churn** — define `fn view() -> Element<_>` consistently and call
`.into()` once at each match arm end rather than inside helpers.
