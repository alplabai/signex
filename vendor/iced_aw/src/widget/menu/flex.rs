//! Distribute elements using a flex-based layout.
// This code is heavily inspired by the [`druid`] codebase.
//
// [`druid`]: https://github.com/xi-editor/druid
//
// Copyright 2018 The xi-editor Authors, Héctor Ramón
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![allow(clippy::all)]
#![allow(clippy::pedantic)]
#![allow(clippy::use_self)]

use iced_core::{
    Alignment, Element, Length, Padding, Pixels, Size, Vector, Widget, layout::Limits, renderer,
    widget,
};

/// The main axis of a flex layout.
#[derive(Debug)]
pub enum Axis {
    /// The vertical axis
    Vertical,
}

impl Axis {
    fn main(&self, size: Size) -> f32 {
        match self {
            Axis::Vertical => size.height,
        }
    }

    fn cross(&self, size: Size) -> f32 {
        match self {
            Axis::Vertical => size.width,
        }
    }

    fn pack<T>(&self, main: T, cross: T) -> (T, T) {
        match self {
            Axis::Vertical => (cross, main),
        }
    }
}

/// Computes the flex layout with the given axis and limits, applying spacing,
/// padding and alignment to the items as needed.
///
/// The layout of each item is stored in its [`widget::Tree`] (size and
/// translation relative to the container), and the resolved [`Size`] of the
/// container is returned.
pub fn resolve<'a, E, T, Message, Theme, Renderer>(
    axis: Axis,
    renderer: &Renderer,
    limits: &Limits,
    width: Length,
    height: Length,
    padding: Padding,
    spacing: Pixels,
    align_items: Alignment,
    items: &mut [E],
    trees: &mut [T],
) -> Size
where
    E: std::borrow::BorrowMut<Element<'a, Message, Theme, Renderer>>,
    T: std::borrow::BorrowMut<widget::Tree>,
    Renderer: renderer::Renderer,
{
    let limits = limits.width(width).height(height).shrink(padding);
    let total_spacing = spacing * items.len().saturating_sub(1) as f32;
    let max_cross = axis.cross(limits.max);

    let mut fill_main_sum = 0;
    let mut cross = match axis {
        Axis::Vertical => match width {
            Length::Shrink => 0.0,
            _ => max_cross,
        },
    };

    let mut available = axis.main(limits.max) - total_spacing.0;

    for (child, tree) in items.iter_mut().zip(trees.iter_mut()) {
        let (fill_main_factor, fill_cross_factor) = {
            let size = child.borrow().size();

            axis.pack(size.width.fill_factor(), size.height.fill_factor())
        };

        if fill_main_factor == 0 {
            if fill_cross_factor == 0 {
                let (max_width, max_height) = axis.pack(available, max_cross);

                let child_limits = Limits::new(Size::ZERO, Size::new(max_width, max_height));

                child
                    .borrow_mut()
                    .layout(tree.borrow_mut(), renderer, &child_limits);
                let size = tree.borrow().size;

                available -= axis.main(size);
                cross = cross.max(axis.cross(size));
            }
        } else {
            fill_main_sum += fill_main_factor;
        }
    }

    for (child, tree) in items.iter_mut().zip(trees.iter_mut()) {
        let (fill_main_factor, fill_cross_factor) = {
            let size = child.borrow().size();

            axis.pack(size.width.fill_factor(), size.height.fill_factor())
        };

        if fill_main_factor == 0 && fill_cross_factor != 0 {
            let (max_width, max_height) = axis.pack(available, cross);

            let child_limits = Limits::new(Size::ZERO, Size::new(max_width, max_height));

            child
                .borrow_mut()
                .layout(tree.borrow_mut(), renderer, &child_limits);
            let size = tree.borrow().size;

            available -= axis.main(size);
            cross = cross.max(axis.cross(size));
        }
    }

    let remaining = match axis {
        Axis::Vertical => match height {
            Length::Shrink => 0.0,
            _ => available.max(0.0),
        },
    };

    for (child, tree) in items.iter_mut().zip(trees.iter_mut()) {
        let (fill_main_factor, fill_cross_factor) = {
            let size = child.borrow().size();

            axis.pack(size.width.fill_factor(), size.height.fill_factor())
        };

        if fill_main_factor != 0 {
            let max_main = remaining * fill_main_factor as f32 / fill_main_sum as f32;

            let min_main = if max_main.is_infinite() {
                0.0
            } else {
                max_main
            };

            let max_cross = if fill_cross_factor == 0 {
                max_cross
            } else {
                cross
            };

            let (min_width, min_height) = axis.pack(min_main, 0.0);
            let (max_width, max_height) = axis.pack(max_main, max_cross);

            let child_limits = Limits::new(
                Size::new(min_width, min_height),
                Size::new(max_width, max_height),
            );

            child
                .borrow_mut()
                .layout(tree.borrow_mut(), renderer, &child_limits);
            cross = cross.max(axis.cross(tree.borrow().size));
        }
    }

    let pad = axis.pack(padding.left, padding.top);
    let mut main = pad.0;

    for (i, tree) in trees.iter_mut().enumerate() {
        let tree = tree.borrow_mut();

        if i > 0 {
            main += spacing.0;
        }

        let (x, y) = axis.pack(main, pad.1);

        let alignment = match axis {
            Axis::Vertical => tree
                .size
                .align(Size::new(cross, 0.0), align_items, Alignment::Start),
        };

        tree.translation = Vector::new(x, y) + alignment;

        main += axis.main(tree.size);
    }

    let (intrinsic_width, intrinsic_height) = axis.pack(main - pad.0, cross);
    let size = limits.resolve(width, height, Size::new(intrinsic_width, intrinsic_height));

    size.expand(padding)
}
