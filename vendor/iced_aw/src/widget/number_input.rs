//! Display fields that can only be filled with numeric type.
//!
//! A [`NumberInput`] has some local [`State`].
use iced_core::{
    Background, Border, Color, Element, Event, Layout, Length, Padding, Point, Rectangle, Shadow,
    Shell, Size, Vector, Widget,
    alignment::Vertical,
    clipboard, keyboard,
    layout::Limits,
    mouse::{self, Cursor},
    renderer,
    shell::Bus,
    text::{LineHeight, Wrapping},
    widget::{
        self, Operation, Tree,
        operation::Focusable,
        tree::{State, Tag},
    },
};
use iced_widget::{Column, Container, Row, Text, text_input};
use num_traits::{Num, NumAssignOps, bounds::Bounded};
use std::{
    fmt::Display,
    ops::{Bound, RangeBounds},
    str::FromStr,
};

use crate::iced_aw_font::advanced_text::{down_open, up_open};
use crate::style::{self, Status};
pub use crate::style::{
    StyleFn,
    number_input::{self, Catalog, Style},
};
use crate::widget::typed_input::TypedInput;

/// The default padding
const DEFAULT_PADDING: Padding = Padding::new(5.0);

/// A field that can only be filled with numeric type.
///
/// # Example
/// ```ignore
/// # use iced_aw::NumberInput;
/// #
/// #[derive(Debug, Clone)]
/// enum Message {
///     NumberInputChanged(u32),
/// }
///
/// let value = 12;
/// let max = 1275;
///
/// let input = NumberInput::new(
///     value,
///     0..=max,
///     Message::NumberInputChanged,
/// )
/// .step(2);
/// ```
#[allow(missing_debug_implementations)]
pub struct NumberInput<'a, T, Message, Theme = iced_widget::Theme, Renderer = iced_widget::Renderer>
where
    Renderer: iced_core::text::Renderer,
    Theme: number_input::ExtendedCatalog,
{
    /// The current value of the [`NumberInput`].
    value: T,
    /// The step for each modify of the [`NumberInput`].
    step: T,
    /// The min value of the [`NumberInput`].
    min: Bound<T>,
    /// The max value of the [`NumberInput`].
    max: Bound<T>,
    /// The content padding of the [`NumberInput`].
    padding: iced_core::Padding,
    /// The text size of the [`NumberInput`].
    size: Option<iced_core::Pixels>,
    /// The underlying element of the [`NumberInput`].
    content: TypedInput<'a, T, InternalMessage<T>, Theme, Renderer>,
    /// The ``on_change`` event of the [`NumberInput`].
    on_change: Option<Box<dyn 'a + Fn(T) -> Message>>,
    /// The ``on_submit`` event of the [`NumberInput`].
    #[allow(clippy::type_complexity)]
    on_submit: Option<Message>,
    /// The ``on_paste`` event of the [`NumberInput`]
    on_paste: Option<Box<dyn 'a + Fn(T) -> Message>>,
    /// The style of the [`NumberInput`].
    class: <Theme as style::number_input::Catalog>::Class<'a>,
    /// The font text of the [`NumberInput`].
    font: iced_core::Font,
    /// The renderer used by the [`NumberInput`].
    _renderer: std::marker::PhantomData<fn() -> Renderer>,
    // /// The Width to use for the ``NumberBox`` Default is ``Length::Fill``
    // width: Length,
    /// Ignore mouse scroll events for the [`NumberInput`] Default is ``false``.
    ignore_scroll_events: bool,
    /// Ignore drawing increase and decrease buttons [`NumberInput`] Default is ``false``.
    ignore_buttons: bool,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::enum_variant_names)]
enum InternalMessage<T> {
    OnChange(T),
    OnSubmit(Result<T, String>),
    OnPaste(T),
}

impl<'a, T, Message, Theme, Renderer> NumberInput<'a, T, Message, Theme, Renderer>
where
    T: Num + NumAssignOps + PartialOrd + Display + FromStr + Clone + Bounded + 'a,
    Message: Clone + 'a,
    Renderer: iced_core::text::Renderer,
    Theme: number_input::ExtendedCatalog,
{
    /// Creates a new [`NumberInput`].
    ///
    /// It expects:
    /// - the current value
    /// - the bound values
    /// - a function that produces a message when the [`NumberInput`] changes
    pub fn new<F>(value: &T, bounds: impl RangeBounds<T>, on_change: F) -> Self
    where
        F: 'static + Fn(T) -> Message + Clone,
        T: 'static,
    {
        let padding = DEFAULT_PADDING;

        Self {
            value: value.clone(),
            step: T::one(),
            min: bounds.start_bound().cloned(),
            max: bounds.end_bound().cloned(),
            padding,
            size: None,
            content: TypedInput::new("", value)
                .on_input(InternalMessage::OnChange)
                .padding(padding)
                .width(Length::Fixed(127.0))
                .class(Theme::default_input()),
            on_change: Some(Box::new(on_change)),
            on_submit: None,
            on_paste: None,
            class: <Theme as style::number_input::Catalog>::default(),
            font: iced_core::Font::default(),
            _renderer: std::marker::PhantomData,
            // width: Length::Shrink,
            ignore_scroll_events: false,
            ignore_buttons: false,
        }
    }

    /// Sets the [`Id`](widget::Id) of the underlying [`TextInput`](iced_widget::TextInput).
    #[must_use]
    pub fn id(mut self, id: impl Into<widget::Id>) -> Self {
        self.content = self.content.id(id.into());
        self
    }

    /// Sets the message that should be produced when some valid text is typed into [`NumberInput`]
    ///
    /// If neither this method nor [`on_submit`](Self::on_submit) is called, the [`NumberInput`] will be disabled
    #[must_use]
    pub fn on_input<F>(mut self, callback: F) -> Self
    where
        F: 'a + Fn(T) -> Message,
    {
        self.content = self.content.on_input(InternalMessage::OnChange);
        self.on_change = Some(Box::new(callback));
        self
    }

    /// Sets the message that should be produced when some text is typed into the [`NumberInput`], if `Some`.
    ///
    /// If this is `None`, and there is no [`on_submit`](Self::on_submit) callback, the [`NumberInput`] will be disabled.
    #[must_use]
    pub fn on_input_maybe<F>(mut self, callback: Option<F>) -> Self
    where
        F: 'a + Fn(T) -> Message,
    {
        if let Some(callback) = callback {
            self.content = self.content.on_input(InternalMessage::OnChange);
            self.on_change = Some(Box::new(callback));
        } else {
            if self.on_submit.is_none() {
                // Used to give a proper type to None, maybe someone can find a better way
                #[allow(unused_assignments)]
                let mut f = Some(InternalMessage::OnChange);
                f = None;
                self.content = self.content.on_input_maybe(f);
            }
            self.on_change = None;
        }
        self
    }

    /// Sets the message that should be produced when the [`NumberInput`] is
    /// focused and the enter key is pressed.
    #[must_use]
    pub fn on_submit(mut self, message: Message) -> Self {
        self.content = self.content.on_submit(InternalMessage::OnSubmit);
        self.on_submit = Some(message);
        self
    }

    /// Sets the message that should be produced when the [`NumbertInput`] is
    /// focused and the enter key is pressed, if `Some`.
    ///
    /// If this is `None`, and there is no [`on_change`](Self::on_input) callback, the [`NumberInput`] will be disabled.
    #[must_use]
    pub fn on_submit_maybe(mut self, message: Option<Message>) -> Self {
        if let Some(message) = message {
            self.content = self.content.on_submit(InternalMessage::OnSubmit);
            self.on_submit = Some(message);
        } else {
            if self.on_change.is_none() {
                // Used to give a proper type to None, maybe someone can find a better way
                #[allow(unused_assignments)]
                let mut f = Some(InternalMessage::OnChange);
                f = None;
                self.content = self.content.on_input_maybe(f);
            }
            // Used to give a proper type to None, maybe someone can find a better way
            #[allow(unused_assignments)]
            let mut f = Some(InternalMessage::OnSubmit);
            f = None;
            self.content = self.content.on_submit_maybe(f);
            self.on_change = None;
        }
        self
    }

    /// Sets the message that should be produced when some text is pasted into the [`NumberInput`], resulting in a valid value
    #[must_use]
    pub fn on_paste<F>(mut self, callback: F) -> Self
    where
        F: 'a + Fn(T) -> Message,
    {
        self.content = self.content.on_paste(InternalMessage::OnPaste);
        self.on_paste = Some(Box::new(callback));
        self
    }

    /// Sets the message that should be produced when some text is pasted into the [`NumberInput`], resulting in a valid value, if `Some`
    #[must_use]
    pub fn on_paste_maybe<F>(mut self, callback: Option<F>) -> Self
    where
        F: 'a + Fn(T) -> Message,
    {
        if let Some(callback) = callback {
            self.content = self.content.on_paste(InternalMessage::OnPaste);
            self.on_paste = Some(Box::new(callback));
        } else {
            // Used to give a proper type to None, maybe someone can find a better way
            #[allow(unused_assignments)]
            let mut f = Some(InternalMessage::OnPaste);
            f = None;
            self.content = self.content.on_paste_maybe(f);
            self.on_paste = None;
        }
        self
    }

    /// Sets the [`Font`] of the [`Text`].
    ///
    /// [`Font`]: iced_core::Font
    /// [`Text`]: iced_widget::Text
    #[allow(clippy::needless_pass_by_value)]
    #[must_use]
    pub fn font(mut self, font: iced_core::Font) -> Self {
        self.font = font;
        self.content = self.content.font(font);
        self
    }

    /// Sets the width of the [`NumberInput`].
    #[must_use]
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.content = self.content.width(width);
        self
    }

    /// Sets the width of the [`NumberInput`].
    #[deprecated(since = "0.11.1", note = "use `width` instead")]
    #[must_use]
    pub fn content_width(self, width: impl Into<Length>) -> Self {
        self.width(width)
    }

    /// Sets the padding of the [`NumberInput`].
    #[must_use]
    pub fn padding(mut self, padding: impl Into<iced_core::Padding>) -> Self {
        let padding = padding.into();
        self.padding = padding;
        self.content = self.content.padding(padding);
        self
    }

    /// Sets the text size of the [`NumberInput`].
    #[must_use]
    pub fn set_size(mut self, size: impl Into<iced_core::Pixels>) -> Self {
        let size = size.into();
        self.size = Some(size);
        self.content = self.content.size(size);
        self
    }

    /// Sets the [`text::LineHeight`](iced_core::text::LineHeight) of the [`NumberInput`].
    #[must_use]
    pub fn line_height(mut self, line_height: impl Into<LineHeight>) -> Self {
        self.content = self.content.line_height(line_height);
        self
    }

    /// Sets the horizontal alignment of the [`NumberInput`].
    #[must_use]
    pub fn align_x(mut self, alignment: impl Into<iced_core::text::Alignment>) -> Self {
        self.content = self.content.align_x(alignment);
        self
    }

    /// Sets the style of the [`NumberInput`].
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        <Theme as style::number_input::Catalog>::Class<'a>: From<StyleFn<'a, Theme, Style>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme, Style>).into();
        self
    }
    /// Sets the style of the input of the [`NumberInput`].
    #[must_use]
    pub fn input_style(
        mut self,
        style: impl Fn(&Theme, text_input::Status) -> text_input::Style + 'a,
    ) -> Self
    where
        <Theme as text_input::Catalog>::Class<'a>: From<text_input::StyleFn<'a, Theme>>,
    {
        self.content = self.content.style(style);
        self
    }

    /// Sets the class of the input of the [`NumberInput`].
    #[must_use]
    pub fn class(
        mut self,
        class: impl Into<<Theme as style::number_input::Catalog>::Class<'a>>,
    ) -> Self {
        self.class = class.into();
        self
    }

    /// Sets the minimum & maximum value (bound) of the [`NumberInput`].
    /// # Example
    /// ```
    /// use iced_aw::widget::number_input;
    /// // Creates a range from -5 till 5.
    /// let input: iced_aw::NumberInput<'_, _, _, iced_widget::Theme, iced_widget::Renderer> = number_input(&4 /* my_value */, 0..=4, |_| () /* my_message */).bounds(-5..=5);
    /// ```
    #[must_use]
    pub fn bounds(mut self, bounds: impl RangeBounds<T>) -> Self {
        self.min = bounds.start_bound().cloned();
        self.max = bounds.end_bound().cloned();

        self
    }

    /// Sets the step of the [`NumberInput`].
    #[must_use]
    pub fn step(mut self, step: T) -> Self {
        self.step = step;
        self
    }

    /// Enable or disable increase and decrease buttons of the [`NumberInput`], by default this is set to
    /// ``false``.
    #[must_use]
    pub fn ignore_buttons(mut self, ignore: bool) -> Self {
        self.ignore_buttons = ignore;
        self
    }

    /// Enable or disable mouse scrolling events of the [`NumberInput`], by default this is set to
    /// ``false``.
    #[must_use]
    pub fn ignore_scroll(mut self, ignore: bool) -> Self {
        self.ignore_scroll_events = ignore;
        self
    }

    /// Checks whether every character of the given text could be part of a number
    /// of the type of the [`NumberInput`].
    fn accepts(&self, text: &str, allow_control: bool) -> bool {
        let supports_negative = self.min() < T::zero();

        text.chars().all(|c| {
            if c.is_control() {
                allow_control
            } else if c.is_ascii_digit() {
                true
            } else if c == '-' {
                supports_negative
            } else {
                let probe = if c == '+' {
                    "+1".to_owned()
                } else {
                    format!("1{c}1")
                };

                T::from_str(&probe).is_ok()
            }
        })
    }

    /// Decrease current value by step of the [`NumberInput`].
    fn decrease_value(&mut self, shell: &mut Shell<Message>) {
        if self.value.clone() > self.min() + self.step.clone()
            && self.valid(&(self.value.clone() - self.step.clone()))
        {
            self.value -= self.step.clone();
        } else if self.value > self.min() {
            self.value = self.min();
        } else {
            return;
        }
        if let Some(on_change) = &self.on_change {
            shell.publish(on_change(self.value.clone()));
        }
    }

    /// Increase current value by step of the [`NumberInput`].
    fn increase_value(&mut self, shell: &mut Shell<Message>) {
        if self.value < self.max() - self.step.clone()
            && self.valid(&(self.value.clone() + self.step.clone()))
        {
            self.value += self.step.clone();
        } else if self.value < self.max() {
            self.value = self.max();
        } else {
            return;
        }
        if let Some(on_change) = &self.on_change {
            shell.publish(on_change(self.value.clone()));
        }
    }

    /// Returns the lower value possible
    /// if the bound is excluded the bound is increased by the step
    fn min(&self) -> T {
        match &self.min {
            Bound::Included(n) => n.clone(),
            Bound::Excluded(n) => n.clone() + self.step.clone(),
            Bound::Unbounded => T::min_value(),
        }
    }

    /// Returns the higher value possible
    /// if the bound is excluded the bound is decreased by the step
    fn max(&self) -> T {
        match &self.max {
            Bound::Included(n) => n.clone(),
            Bound::Excluded(n) => n.clone() - self.step.clone(),
            Bound::Unbounded => T::max_value(),
        }
    }

    /// Checks if the value is within the bounds
    fn valid(&self, value: &T) -> bool {
        (match &self.min {
            Bound::Included(n) if *n > *value => false,
            Bound::Excluded(n) if *n >= *value => false,
            _ => true,
        }) && (match &self.max {
            Bound::Included(n) if *n < *value => false,
            Bound::Excluded(n) if *n <= *value => false,
            _ => true,
        })
    }

    /// Checks if the value can be increased by the step
    fn can_increase(&self) -> bool {
        (self.value < self.max() - self.step.clone()
            && self.valid(&(self.value.clone() + self.step.clone())))
            || self.value < self.max()
    }

    /// Checks if the value can be decreased by the step
    fn can_decrease(&self) -> bool {
        (self.value.clone() > self.min() + self.step.clone()
            && self.valid(&(self.value.clone() - self.step.clone())))
            || self.value > self.min()
    }

    /// Checks if the [`NumberInput`] is disabled
    /// Meaning that the bounds are too tight for the value to change
    fn disabled(&self) -> bool {
        match (&self.min, &self.max) {
            (Bound::Included(n) | Bound::Excluded(n), Bound::Included(m) | Bound::Excluded(m)) => {
                *n >= *m
            }
            _ => false,
        }
    }
}

/// Probes whether the text input inside a [`NumberInput`] is focused.
#[derive(Default)]
struct FocusProbe {
    focused: bool,
}

impl Operation for FocusProbe {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }

    fn focusable(
        &mut self,
        _id: Option<&widget::Id>,
        _bounds: Rectangle,
        state: &mut dyn Focusable,
    ) {
        self.focused = state.is_focused();
    }
}

impl<'a, T, Message, Theme, Renderer> widget::Meta for NumberInput<'a, T, Message, Theme, Renderer>
where
    Renderer: iced_core::text::Renderer,
    Theme: number_input::ExtendedCatalog,
{
}

impl<'a, T, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for NumberInput<'a, T, Message, Theme, Renderer>
where
    T: Num + NumAssignOps + PartialOrd + Display + FromStr + ToString + Clone + Bounded + 'a,
    Message: 'a + Clone,
    Renderer: 'static + iced_core::text::Renderer,
    Theme: number_input::ExtendedCatalog,
{
    fn tag(&self) -> Tag {
        Tag::of::<ModifierState>()
    }
    fn state(&self) -> State {
        State::new(ModifierState::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        // Child 0 is the underlying input; child 1 (the modifier buttons) is
        // created and reconciled during layout.
        tree.children.truncate(2);

        if let Some(content) = tree.children.get_mut(0) {
            content.diff::<InternalMessage<T>, Theme, Renderer>(&mut self.content);
        } else {
            let mut content = Tree::new::<InternalMessage<T>, Theme, Renderer>(&self.content);
            content.diff::<InternalMessage<T>, Theme, Renderer>(&mut self.content);
            tree.children.push(content);
        }
    }

    fn size(&self) -> Size<Length> {
        <TypedInput<'a, T, InternalMessage<T>, Theme, Renderer> as Widget<
            InternalMessage<T>,
            Theme,
            Renderer,
        >>::size(&self.content)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) {
        let num_size = Widget::<Message, Theme, Renderer>::size(self);
        let limits = limits.width(num_size.width).height(Length::Fit);
        Widget::<InternalMessage<T>, Theme, Renderer>::layout(
            &mut self.content,
            &mut tree.children[0],
            renderer,
            &limits,
        );
        let content_size = tree.children[0].size;
        let limits2 = Limits::new(Size::ZERO, content_size);
        let txt_size = self.size.unwrap_or_else(|| renderer.text_size());

        let icon_size = txt_size * 2.5 / 4.0;
        let btn_mod = |c| {
            Container::<Text<'_, Theme>, Theme>::new(Text::new(format!(" {c} ")).size(icon_size))
                .center_y(Length::Fit)
                .center_x(Length::Fit)
        };

        let default_padding = DEFAULT_PADDING;

        let mut element: Element<'_, Message, Theme, Renderer> = if self.padding.top
            < default_padding.top
            || self.padding.bottom < default_padding.bottom
            || self.padding.right < default_padding.right
        {
            Widget::<Message, Theme, Renderer>::_boxed(
                Row::new()
                    .spacing(1.0)
                    .width(Length::Fit)
                    .push(btn_mod('+'))
                    .push(btn_mod('-')),
            )
        } else {
            Widget::<Message, Theme, Renderer>::_boxed(
                Column::new()
                    .spacing(1.0)
                    .width(Length::Fit)
                    .push(btn_mod('▲'))
                    .push(btn_mod('▼')),
            )
        };

        {
            let modifier_tree = if let Some(child_tree) = tree.children.get_mut(1) {
                child_tree.diff(&mut element);
                child_tree
            } else {
                let child_tree = Tree::new(&element);
                tree.children.insert(1, child_tree);
                &mut tree.children[1]
            };

            element.layout(modifier_tree, renderer, &limits2.loose());
        }

        let modifier_size = tree.children[1].size;
        let intrinsic = Size::new(
            content_size.width - 1.0,
            content_size.height.max(modifier_size.height),
        );

        // The modifier buttons are aligned to the end (right) and centered vertically.
        tree.children[0].translation = Vector::ZERO;
        tree.children[1].translation = Vector::new(
            intrinsic.width - modifier_size.width,
            (intrinsic.height - modifier_size.height) / 2.0,
        );

        tree.size = limits.resolve(num_size.width, Length::Fit, intrinsic);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds(), viewport);

        operation.traverse(&mut |operation| {
            let mut children = layout.iter_mut(&mut tree.children);

            // Operate on the text input (first child)
            if let Some((content_layout, content_tree)) = children.next() {
                Widget::<InternalMessage<T>, Theme, Renderer>::operate(
                    &mut self.content,
                    content_tree,
                    content_layout,
                    viewport,
                    renderer,
                    operation,
                );
            }

            // Operate on the modifier buttons (second child) to expose them for testing
            // The buttons are rendered as text elements, so we need to recreate their structure
            if let Some((modifier_layout, modifier_tree)) = children.next()
                && !self.ignore_buttons
            {
                let txt_size = self.size.unwrap_or_else(|| renderer.text_size());
                let icon_size = txt_size * 2.5 / 4.0;

                let btn_mod = |c| {
                    Container::<Text<'_, Theme>, Theme>::new(
                        Text::new(format!(" {c} ")).size(icon_size),
                    )
                    .center_y(Length::Fit)
                    .center_x(Length::Fit)
                };

                let default_padding = DEFAULT_PADDING;

                // Recreate the modifier element structure to operate on it
                let mut element: Element<'_, Message, Theme, Renderer> = if self.padding.top
                    < default_padding.top
                    || self.padding.bottom < default_padding.bottom
                    || self.padding.right < default_padding.right
                {
                    Widget::<Message, Theme, Renderer>::_boxed(
                        Row::new()
                            .spacing(1.0)
                            .width(Length::Fit)
                            .push(btn_mod('+'))
                            .push(btn_mod('-')),
                    )
                } else {
                    Widget::<Message, Theme, Renderer>::_boxed(
                        Column::new()
                            .spacing(1.0)
                            .width(Length::Fit)
                            .push(btn_mod('▲'))
                            .push(btn_mod('▼')),
                    )
                };

                // Operate on the modifier element to expose button text
                element.operate(
                    modifier_tree,
                    modifier_layout,
                    viewport,
                    renderer,
                    operation,
                );
            }
        });
    }

    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let Tree {
            state, children, ..
        } = tree;

        let mut layouts = layout.iter_mut(children);
        let (content_layout, content_tree) = layouts.next().expect("fail to get content layout");
        let (modifier_layout, modifier_tree) =
            layouts.next().expect("fail to get modifiers layout");
        let mut mod_children = modifier_layout.iter(&modifier_tree.children);
        let inc_bounds = mod_children
            .next()
            .expect("fail to get increase mod layout")
            .0
            .bounds();
        let dec_bounds = mod_children
            .next()
            .expect("fail to get decrease mod layout")
            .0
            .bounds();

        if self.disabled() {
            return;
        }
        let can_decrease = self.can_decrease();
        let can_increase = self.can_increase();

        let cursor_position = cursor.position().unwrap_or_default();
        let mouse_over_widget = layout.bounds().contains(cursor_position);
        let mouse_over_inc = inc_bounds.contains(cursor_position);
        let mouse_over_dec = dec_bounds.contains(cursor_position);
        let mouse_over_button = mouse_over_inc || mouse_over_dec;

        let mod_state = state.downcast_mut::<ModifierState>();

        // Whether the event must be forwarded to the underlying [`TypedInput`]
        let mut forward = false;

        match event {
            Event::Keyboard(key) => {
                let mut probe = FocusProbe::default();
                Widget::<InternalMessage<T>, Theme, Renderer>::operate(
                    &mut self.content,
                    content_tree,
                    content_layout,
                    viewport,
                    renderer,
                    &mut probe,
                );

                if !probe.focused {
                    return;
                }

                match key {
                    keyboard::Event::ModifiersChanged(_) => forward = true,
                    keyboard::Event::KeyReleased { .. } => return,
                    keyboard::Event::KeyPressed {
                        key,
                        text,
                        modifiers,
                        ..
                    } => {
                        // If true, ignore Arrow/Home/End keys - they are coming from numpad and are just
                        // mislabeled. See the core PR:
                        // https://github.com/iced-rs/iced/pull/2278
                        let has_value = !modifiers.command()
                            && text
                                .as_ref()
                                .is_some_and(|t| t.chars().any(|c| !c.is_control()));

                        match key.as_ref() {
                            // Arrow Down, decrease by step
                            keyboard::Key::Named(keyboard::key::Named::ArrowDown)
                                if can_decrease && !has_value =>
                            {
                                shell.capture_event();
                                shell.request_redraw();
                                self.decrease_value(shell);
                            }
                            // Arrow Up, increase by step
                            keyboard::Key::Named(keyboard::key::Named::ArrowUp)
                                if can_increase && !has_value =>
                            {
                                shell.capture_event();
                                shell.request_redraw();

                                self.increase_value(shell);
                            }
                            // Everything else is handled by the underlying input, as long as
                            // the text it would insert can be part of a number.
                            _ => {
                                if !modifiers.command()
                                    && let Some(text) = text
                                    && !self.accepts(text, true)
                                {
                                    shell.capture_event();
                                    return;
                                }

                                forward = true;
                            }
                        }
                    }
                }
            }
            // Pasted text coming from the clipboard
            Event::Clipboard(clipboard::Event::Read(result)) => {
                let mut probe = FocusProbe::default();
                Widget::<InternalMessage<T>, Theme, Renderer>::operate(
                    &mut self.content,
                    content_tree,
                    content_layout,
                    viewport,
                    renderer,
                    &mut probe,
                );

                if probe.focused
                    && let Ok(content) = result
                    && let clipboard::Content::Text(text) = content.as_ref()
                    && !self.accepts(text, false)
                {
                    shell.capture_event();
                    return;
                }

                forward = true;
            }
            // Mouse scroll event
            Event::Mouse(mouse::Event::WheelScrolled { delta })
                if mouse_over_widget && !self.ignore_scroll_events =>
            {
                match delta {
                    mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => {
                        if y.is_sign_positive() {
                            self.increase_value(shell);
                        } else {
                            self.decrease_value(shell);
                        }
                    }
                }
                shell.capture_event();
                shell.request_redraw();
            }
            // Clicking on the buttons up or down
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if mouse_over_button && !self.ignore_buttons =>
            {
                if mouse_over_dec {
                    mod_state.decrease_pressed = true;
                    self.decrease_value(shell);
                } else {
                    mod_state.increase_pressed = true;
                    self.increase_value(shell);
                }
                shell.capture_event();
                shell.request_redraw();
            }
            // Releasing the buttons
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if mouse_over_button =>
            {
                if mouse_over_dec {
                    mod_state.decrease_pressed = false;
                } else {
                    mod_state.increase_pressed = false;
                }
                shell.capture_event();
                shell.request_redraw();
            }
            // Any other event are just forwarded
            _ => forward = true,
        }

        if !forward {
            return;
        }

        // We use a secondary shell to handle the event of the underlying [`TypedInput`]
        let mut bus = Bus::new();
        let mut sub_shell = shell.local(&mut bus);

        Widget::<InternalMessage<T>, Theme, Renderer>::update(
            &mut self.content,
            content_tree,
            event,
            content_layout,
            cursor,
            renderer,
            &mut sub_shell,
            viewport,
        );

        // We forward the shell of the [`TypedInput`] to the application
        shell.request_redraw_at(sub_shell.redraw_request());
        shell.invalidate(sub_shell.invalidation());
        shell.request_input_method(sub_shell.input_method());

        if sub_shell.is_event_captured() {
            shell.capture_event();
        }

        let mut sub_clipboard = std::mem::take(sub_shell.clipboard_mut());
        shell.clipboard_mut().merge(&mut sub_clipboard);

        // An emptied input (or a lone minus sign) resets the value to zero
        if matches!(event, Event::Keyboard(_) | Event::Clipboard(_)) {
            let text = self.content.text();

            if text.is_empty() || (text == "-" && self.min() < T::zero()) {
                self.value = T::zero();
            }
        }

        for (message, receipt) in bus.drain() {
            match message {
                InternalMessage::OnChange(value) => {
                    // Values out of bounds are discarded and the widgets rebuilt,
                    // which restores the text of the input.
                    if !self.valid(&value) {
                        shell.invalidate_widgets();
                        continue;
                    }

                    if self.value != value || self.value.is_zero() {
                        self.value = value.clone();
                        if let Some(on_change) = &self.on_change {
                            shell.forward(on_change(value), receipt);
                        }
                    }
                    shell.invalidate_layout();
                }
                InternalMessage::OnSubmit(result) => {
                    if let Err(text) = result {
                        assert!(
                            text.is_empty(),
                            "We shouldn't be able to submit a number input with an invalid value"
                        );
                    }
                    if let Some(on_submit) = &self.on_submit {
                        shell.forward(on_submit.clone(), receipt);
                    }
                    shell.invalidate_layout();
                }
                InternalMessage::OnPaste(value) => {
                    if !self.valid(&value) {
                        shell.invalidate_widgets();
                        continue;
                    }

                    if self.value != value {
                        self.value = value.clone();
                        if let Some(on_paste) = &self.on_paste {
                            shell.forward(on_paste(value), receipt);
                        }
                    }
                    shell.invalidate_layout();
                }
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();
        let mut children = layout.iter(&tree.children);
        let _content_layout = children.next().expect("fail to get content layout");
        let (modifier_layout, modifier_tree) =
            children.next().expect("fail to get modifiers layout");
        let mut mod_children = modifier_layout.iter(&modifier_tree.children);
        let inc_bounds = mod_children
            .next()
            .expect("fail to get increase mod layout")
            .0
            .bounds();
        let dec_bounds = mod_children
            .next()
            .expect("fail to get decrease mod layout")
            .0
            .bounds();
        let is_mouse_over = bounds.contains(cursor.position().unwrap_or_default());
        let is_decrease_disabled = !self.can_decrease();
        let is_increase_disabled = !self.can_increase();
        let mouse_over_decrease = dec_bounds.contains(cursor.position().unwrap_or_default());
        let mouse_over_increase = inc_bounds.contains(cursor.position().unwrap_or_default());

        if ((mouse_over_decrease && !is_decrease_disabled)
            || (mouse_over_increase && !is_increase_disabled))
            && !self.ignore_buttons
        {
            mouse::Interaction::Pointer
        } else if is_mouse_over {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let mut children = layout.iter(&tree.children);
        let (content_layout, content_tree) = children.next().expect("fail to get content layout");
        let (modifier_layout, modifier_tree) =
            children.next().expect("fail to get modifiers layout");
        let mut mod_children = modifier_layout.iter(&modifier_tree.children);
        let inc_bounds = mod_children
            .next()
            .expect("fail to get increase mod layout")
            .0
            .bounds();
        let dec_bounds = mod_children
            .next()
            .expect("fail to get decrease mod layout")
            .0
            .bounds();
        Widget::<InternalMessage<T>, Theme, Renderer>::draw(
            &self.content,
            content_tree,
            renderer,
            theme,
            style,
            content_layout,
            cursor,
            viewport,
        );
        let is_decrease_disabled = !self.can_decrease();
        let is_increase_disabled = !self.can_increase();

        let decrease_btn_style = if is_decrease_disabled {
            style::number_input::Catalog::style(theme, &self.class, Status::Disabled)
        } else if tree.state.downcast_ref::<ModifierState>().decrease_pressed {
            style::number_input::Catalog::style(theme, &self.class, Status::Pressed)
        } else {
            style::number_input::Catalog::style(theme, &self.class, Status::Active)
        };

        let increase_btn_style = if is_increase_disabled {
            style::number_input::Catalog::style(theme, &self.class, Status::Disabled)
        } else if tree.state.downcast_ref::<ModifierState>().increase_pressed {
            style::number_input::Catalog::style(theme, &self.class, Status::Pressed)
        } else {
            style::number_input::Catalog::style(theme, &self.class, Status::Active)
        };

        let txt_size = self.size.unwrap_or_else(|| renderer.text_size());

        let icon_size = txt_size * 2.5 / 4.0;

        if self.ignore_buttons {
            return;
        }
        // decrease button section
        if dec_bounds.intersects(viewport) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: dec_bounds,
                    border: Border {
                        radius: (3.0).into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                        ..Border::default()
                    },
                    shadow: Shadow::default(),
                    snap: false,
                },
                decrease_btn_style
                    .button_background
                    .unwrap_or(Background::Color(Color::TRANSPARENT)),
            );
        }

        let (content, font, shaping) = down_open();
        renderer.fill_text(
            iced_core::text::Text {
                content,
                bounds: Size::new(dec_bounds.width, dec_bounds.height),
                size: icon_size,
                font,
                line_height: LineHeight::Relative(1.3),
                shaping,
                wrapping: Wrapping::default(),
                align_x: iced_core::text::Alignment::Center,
                align_y: Vertical::Center,
                ellipsis: iced_core::text::Ellipsis::None,
                hint_factor: renderer.hint_factor(),
            },
            Point::new(dec_bounds.center_x(), dec_bounds.center_y()),
            decrease_btn_style.icon_color,
            dec_bounds,
        );

        // increase button section
        if inc_bounds.intersects(viewport) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: inc_bounds,
                    border: Border {
                        radius: (3.0).into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                        ..Border::default()
                    },
                    shadow: Shadow::default(),
                    snap: false,
                },
                increase_btn_style
                    .button_background
                    .unwrap_or(Background::Color(Color::TRANSPARENT)),
            );
        }

        let (content, font, shaping) = up_open();
        renderer.fill_text(
            iced_core::text::Text {
                content,
                bounds: Size::new(inc_bounds.width, inc_bounds.height),
                size: icon_size,
                font,
                line_height: LineHeight::Relative(1.3),
                shaping,
                wrapping: Wrapping::default(),
                align_x: iced_core::text::Alignment::Center,
                align_y: Vertical::Center,
                ellipsis: iced_core::text::Ellipsis::None,
                hint_factor: renderer.hint_factor(),
            },
            Point::new(inc_bounds.center_x(), inc_bounds.center_y()),
            increase_btn_style.icon_color,
            inc_bounds,
        );
    }
}

/// The modifier state of a [`NumberInput`].
#[derive(Default, Clone, Debug)]
pub struct ModifierState {
    /// The state of decrease button on a [`NumberInput`].
    pub decrease_pressed: bool,
    /// The state of increase button on a [`NumberInput`].
    pub increase_pressed: bool,
}

impl<'a, T, Message, Theme, Renderer> From<NumberInput<'a, T, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    T: 'a + Num + NumAssignOps + PartialOrd + Display + FromStr + Clone + Bounded,
    Message: 'a + Clone,
    Renderer: 'static + iced_core::text::Renderer,
    Theme: 'a + number_input::ExtendedCatalog,
{
    fn from(num_input: NumberInput<'a, T, Message, Theme, Renderer>) -> Self {
        Widget::<Message, Theme, Renderer>::_boxed(num_input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced_widget::Renderer;

    #[derive(Clone, Debug)]
    #[allow(dead_code)]
    enum TestMessage {
        Changed(u32),
        Submit,
    }

    type TestNumberInput<'a> = NumberInput<'a, u32, TestMessage, iced_widget::Theme, Renderer>;

    #[test]
    fn number_input_new_creates_instance() {
        let value = 10u32;
        let input = TestNumberInput::new(&value, 0..=100, TestMessage::Changed);

        assert_eq!(input.value, 10);
        assert_eq!(input.step, 1);
        assert!(matches!(input.min, Bound::Included(0)));
        assert!(matches!(input.max, Bound::Included(100)));
        assert!(!input.ignore_scroll_events);
        assert!(!input.ignore_buttons);
    }

    #[test]
    fn number_input_with_step() {
        let value = 10u32;
        let input = TestNumberInput::new(&value, 0..=100, TestMessage::Changed).step(5);

        assert_eq!(input.step, 5);
    }

    #[test]
    fn number_input_ignore_buttons() {
        let value = 10u32;
        let input =
            TestNumberInput::new(&value, 0..=100, TestMessage::Changed).ignore_buttons(true);

        assert!(input.ignore_buttons);
    }

    #[test]
    fn number_input_ignore_scroll() {
        let value = 10u32;
        let input = TestNumberInput::new(&value, 0..=100, TestMessage::Changed).ignore_scroll(true);

        assert!(input.ignore_scroll_events);
    }

    #[test]
    fn number_input_bounds() {
        let value = 50u32;
        let input = TestNumberInput::new(&value, 0..=100, TestMessage::Changed).bounds(10..=90);

        assert!(matches!(input.min, Bound::Included(10)));
        assert!(matches!(input.max, Bound::Included(90)));
    }

    #[test]
    fn number_input_can_increase() {
        let value = 50u32;
        let input = TestNumberInput::new(&value, 0..=100, TestMessage::Changed).step(10);

        assert!(input.can_increase());
    }

    #[test]
    fn number_input_cannot_increase_at_max() {
        let value = 100u32;
        let input = TestNumberInput::new(&value, 0..=100, TestMessage::Changed);

        assert!(!input.can_increase());
    }

    #[test]
    fn number_input_can_decrease() {
        let value = 50u32;
        let input = TestNumberInput::new(&value, 0..=100, TestMessage::Changed).step(10);

        assert!(input.can_decrease());
    }

    #[test]
    fn number_input_cannot_decrease_at_min() {
        let value = 0u32;
        let input = TestNumberInput::new(&value, 0..=100, TestMessage::Changed);

        assert!(!input.can_decrease());
    }

    #[test]
    fn number_input_valid_value() {
        let value = 50u32;
        let input = TestNumberInput::new(&value, 0..=100, TestMessage::Changed);

        assert!(input.valid(&50));
        assert!(input.valid(&0));
        assert!(input.valid(&100));
        assert!(!input.valid(&150));
    }

    #[test]
    fn number_input_min_max_values() {
        let value = 50u32;
        let input = TestNumberInput::new(&value, 10..=90, TestMessage::Changed);

        assert_eq!(input.min(), 10);
        assert_eq!(input.max(), 90);
    }

    #[test]
    fn number_input_min_max_with_excluded_bounds() {
        let value = 50u32;
        let input = TestNumberInput::new(&value, 10..90, TestMessage::Changed).step(1);

        // Range 10..90 means start is Included(10), end is Excluded(90)
        assert_eq!(input.min(), 10); // Included bound
        assert_eq!(input.max(), 89); // Excluded bound - step
    }

    #[test]
    fn number_input_disabled_when_bounds_too_tight() {
        let value = 50u32;
        // When min == max (50..=50), the widget is disabled because there's no room to change
        let input = TestNumberInput::new(&value, 50..=50, TestMessage::Changed);
        assert!(input.disabled());

        // When min < max, the widget is not disabled
        let input = TestNumberInput::new(&value, 49..=50, TestMessage::Changed);
        assert!(!input.disabled());
    }

    #[test]
    fn number_input_tag_returns_modifier_state_tag() {
        let value = 10u32;
        let input = TestNumberInput::new(&value, 0..=100, TestMessage::Changed);

        let tag = Widget::<TestMessage, iced_widget::Theme, Renderer>::tag(&input);
        assert_eq!(tag, Tag::of::<ModifierState>());
    }

    #[test]
    fn number_input_has_one_child() {
        let value = 10u32;
        let mut input = TestNumberInput::new(&value, 0..=100, TestMessage::Changed);

        let mut tree = Tree::new::<TestMessage, iced_widget::Theme, Renderer>(&input);
        Widget::<TestMessage, iced_widget::Theme, Renderer>::diff(&mut input, &mut tree);
        assert_eq!(tree.children.len(), 1); // Only the content (TypedInput) is a child initially
    }

    #[test]
    fn number_input_different_values() {
        let test_values = [(0, 0..=100), (50, 0..=100), (100, 0..=100), (25, 10..=50)];

        for (value, range) in test_values {
            let input = TestNumberInput::new(&value, range, TestMessage::Changed);
            assert_eq!(input.value, value);
        }
    }

    #[test]
    fn modifier_state_defaults() {
        let state = ModifierState::default();

        assert!(!state.decrease_pressed);
        assert!(!state.increase_pressed);
    }

    #[test]
    fn number_input_with_on_submit() {
        let value = 10u32;
        let input = TestNumberInput::new(&value, 0..=100, TestMessage::Changed)
            .on_submit(TestMessage::Submit);

        assert!(input.on_submit.is_some());
    }

    #[test]
    fn number_input_padding() {
        let value = 10u32;
        let custom_padding = Padding::new(10.0);
        let input =
            TestNumberInput::new(&value, 0..=100, TestMessage::Changed).padding(custom_padding);

        assert_eq!(input.padding, custom_padding);
    }

    #[test]
    fn number_input_size() {
        let value = 10u32;
        let input = TestNumberInput::new(&value, 0..=100, TestMessage::Changed).set_size(20.0);

        assert_eq!(input.size, Some(iced_core::Pixels(20.0)));
    }

    #[test]
    fn number_input_width() {
        let value = 10u32;
        let _input = TestNumberInput::new(&value, 0..=100, TestMessage::Changed).width(200);

        // We can't easily verify the width was set since it's stored in the content widget
        // This test just ensures the method doesn't panic
    }
}
