//! Multi-channel sheets (#1279): a sheet symbol whose designator reads
//! `Repeat(CH, 1, 4)` is placed once and compiles to four channels.
//!
//! The repeat lives in text the file already carries, so a sheet that uses it
//! needs no new field and an older build reads a plain sheet symbol:
//!
//! * the sheet symbol's designator (`ChildSheet.name`) is `Repeat(<channel>,
//!   <from>, <to>)`;
//! * a sheet entry named `Repeat(<net>)` gives one net per channel and meets a
//!   bus on the parent sheet; a plain entry is shared by every channel.
//!
//! How channel designators and channel-local net names are spelled is a
//! project setting, [`ChannelNaming`]. The model and the stitching rules are
//! in `docs/MULTI_CHANNEL.md`.

use thiserror::Error;

use crate::project::ChannelNaming;

/// The keyword, case-sensitive.
const KEYWORD: &str = "Repeat";

/// Most channels one repeated sheet may compile to.
pub const MAX_CHANNELS: u32 = 64;

/// A parsed `Repeat(CH, 1, 4)` sheet-symbol designator.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChannelRepeat {
    /// The channel name stem (`CH`).
    pub channel: String,
    /// First channel number.
    pub from: u32,
    /// Last channel number, `from <= to`.
    pub to: u32,
}

/// One channel of a repeated sheet.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChannelOccurrence {
    /// The channel's name: stem plus number (`CH1`).
    pub name: String,
    /// 1-based position among the channels.
    pub index: u32,
    /// The channel number, in `from..=to`.
    pub number: u32,
    /// How many channels the sheet compiles to.
    pub of: u32,
}

/// Why a custom channel naming format is refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ChannelFormatError {
    #[error("the format is empty")]
    Empty,
    #[error("the format contains whitespace")]
    Whitespace,
    #[error("a token opened with '${{' is not closed with '}}'")]
    Unclosed,
    #[error("unknown token '${{{0}}}'")]
    UnknownToken(String),
    #[error("the format has no channel token, so every channel would get the same name")]
    NoChannelToken,
    #[error("the format has no ${{Designator}} or ${{Net}} token")]
    NoBaseToken,
}

impl ChannelRepeat {
    /// Parse a sheet-symbol designator. Whitespace around the parts is
    /// allowed; the keyword is case-sensitive; the channel stem is an
    /// identifier; `from <= to` and at most [`MAX_CHANNELS`] channels.
    pub fn parse(designator: &str) -> Option<Self> {
        let inner = call_argument(designator)?;
        let mut parts = inner.split(',').map(str::trim);
        let channel = parts.next()?;
        let from = parts.next()?.parse::<u32>().ok()?;
        let to = parts.next()?.parse::<u32>().ok()?;
        if parts.next().is_some() || !is_identifier(channel) || from > to {
            return None;
        }
        if to - from >= MAX_CHANNELS {
            return None;
        }
        Some(Self {
            channel: channel.to_string(),
            from,
            to,
        })
    }

    /// The canonical designator text, `Repeat(CH, 1, 4)`.
    pub fn to_designator(&self) -> String {
        format!("{KEYWORD}({}, {}, {})", self.channel, self.from, self.to)
    }

    /// Number of channels.
    pub fn count(&self) -> u32 {
        self.to - self.from + 1
    }

    /// Every channel, in `from..=to` order.
    pub fn occurrences(&self) -> Vec<ChannelOccurrence> {
        let of = self.count();
        (self.from..=self.to)
            .enumerate()
            .map(|(i, number)| ChannelOccurrence {
                name: format!("{}{number}", self.channel),
                index: i as u32 + 1,
                number,
                of,
            })
            .collect()
    }
}

/// The net a `Repeat(<net>)` sheet-entry name repeats per channel, or `None`
/// for a plain (shared) entry.
pub fn repeated_entry_name(name: &str) -> Option<&str> {
    let inner = call_argument(name)?.trim();
    let plain = !inner.is_empty()
        && !inner
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, ',' | '(' | ')'));
    plain.then_some(inner)
}

/// The text between `Repeat(` and the closing `)`, whitespace-tolerant.
fn call_argument(text: &str) -> Option<&str> {
    let rest = text.trim().strip_prefix(KEYWORD)?.trim_start();
    rest.strip_prefix('(')?.strip_suffix(')')
}

fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Spreadsheet-style letters for a 1-based position: 1 is `A`, 26 is `Z`,
/// 27 is `AA`.
pub fn channel_alpha(index: u32) -> String {
    let mut n = index;
    let mut out = Vec::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        out.push(b'A' + rem as u8);
        n = (n - 1) / 26;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// One outline drawn behind a repeated sheet symbol: `[x0, y0, x1, y1]` in
/// mm, and the alpha its stroke is drawn at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StackLayer {
    pub rect: [f64; 4],
    pub alpha: f32,
}

/// How a repeated sheet symbol is drawn on the canvas and in PDF and SVG
/// output: two outlines behind it and a ×N pill at its top-right corner.
#[derive(Debug, Clone, PartialEq)]
pub struct RepeatStack {
    /// Back to front.
    pub layers: [StackLayer; 2],
    /// The pill, `[x0, y0, x1, y1]` in mm.
    pub badge: [f64; 4],
    /// `×4`.
    pub badge_text: String,
    pub badge_text_mm: f64,
}

/// Offsets of the outlines behind the symbol (one and two half-grid steps)
/// and their alphas, back to front.
const STACK: [(f64, f32); 2] = [(2.54, 0.45), (1.27, 0.7)];
/// The pill's size and its inset from the symbol's top-right corner, mm.
const BADGE_W: f64 = 5.08;
const BADGE_H: f64 = 2.286;
const BADGE_INSET: f64 = 0.762;
const BADGE_TEXT_MM: f64 = 1.27;

impl RepeatStack {
    /// The stack of a sheet symbol at `position` of `size`, repeated
    /// `count` times.
    pub fn new(position: (f64, f64), size: (f64, f64), count: u32) -> Self {
        let (x0, y0) = position;
        let (x1, y1) = (x0 + size.0, y0 + size.1);
        let layer = |(d, alpha): (f64, f32)| StackLayer {
            rect: [x0 + d, y0 + d, x1 + d, y1 + d],
            alpha,
        };
        let (bx1, by0) = (x1.max(x0) - BADGE_INSET, y0.min(y1) + BADGE_INSET);
        Self {
            layers: [layer(STACK[0]), layer(STACK[1])],
            badge: [bx1 - BADGE_W, by0, bx1, by0 + BADGE_H],
            badge_text: format!("×{count}"),
            badge_text_mm: BADGE_TEXT_MM,
        }
    }

    /// The stack of `sheet`, or `None` when its designator is not a repeat.
    pub fn of(sheet: &super::ChildSheet) -> Option<Self> {
        let repeat = ChannelRepeat::parse(&sheet.name)?;
        let position = (sheet.position.x, sheet.position.y);
        Some(Self::new(position, sheet.size, repeat.count()))
    }
}

/// Token names a naming format may use.
const CHANNEL_TOKENS: [&str; 3] = ["Channel", "Index", "ChannelAlpha"];
const BASE_TOKENS: [&str; 2] = ["Designator", "Net"];

impl ChannelNaming {
    /// The designator `base` takes in `channel` (`R5` in CH1 is `CH1_R5`
    /// with the default format).
    pub fn designator(&self, base: &str, channel: &ChannelOccurrence) -> String {
        expand(&self.designator_format, channel, ("Designator", base))
    }

    /// The name a channel-local net `net` takes in `channel` (`VOUT_3V3` in
    /// CH1 is `VOUT_3V3_CH1` with the default format).
    pub fn net(&self, net: &str, channel: &ChannelOccurrence) -> String {
        expand(&self.net_format, channel, ("Net", net))
    }
}

/// Replace every known `${Token}` of `format`; an unknown or unclosed token
/// stays as written.
fn expand(format: &str, channel: &ChannelOccurrence, base: (&str, &str)) -> String {
    let mut out = String::with_capacity(format.len() + 8);
    let mut rest = format;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let token = &after[..end];
        match token_value(token, channel, base) {
            Some(value) => out.push_str(&value),
            None => out.push_str(&rest[start..start + 2 + end + 1]),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

fn token_value(token: &str, channel: &ChannelOccurrence, base: (&str, &str)) -> Option<String> {
    match token {
        "Channel" => Some(channel.name.clone()),
        "Index" => Some(channel.index.to_string()),
        "ChannelAlpha" => Some(channel_alpha(channel.index)),
        t if t == base.0 => Some(base.1.to_string()),
        _ => None,
    }
}

/// Check a custom naming format: not empty, no whitespace, every token closed
/// and known, at least one channel token and one of `${Designator}` /
/// `${Net}`.
pub fn check_format(text: &str) -> Result<(), ChannelFormatError> {
    if text.is_empty() {
        return Err(ChannelFormatError::Empty);
    }
    if text.chars().any(char::is_whitespace) {
        return Err(ChannelFormatError::Whitespace);
    }
    let (mut has_channel, mut has_base) = (false, false);
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        let after = &rest[start + 2..];
        let end = after.find('}').ok_or(ChannelFormatError::Unclosed)?;
        let token = &after[..end];
        if CHANNEL_TOKENS.contains(&token) {
            has_channel = true;
        } else if BASE_TOKENS.contains(&token) {
            has_base = true;
        } else {
            return Err(ChannelFormatError::UnknownToken(token.to_string()));
        }
        rest = &after[end + 1..];
    }
    if !has_channel {
        return Err(ChannelFormatError::NoChannelToken);
    }
    if !has_base {
        return Err(ChannelFormatError::NoBaseToken);
    }
    Ok(())
}
