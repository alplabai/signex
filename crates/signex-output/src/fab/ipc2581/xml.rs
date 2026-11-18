//! A small XML writer: elements with attributes, two-space indent, no text
//! nodes (IPC-2581 carries everything in attributes). Attribute order is the
//! caller's, so one board gives the same bytes.

/// The attributes of one element, in the order they are written.
pub(crate) type Attrs<'a> = &'a [(&'a str, String)];

pub(crate) struct Xml {
    out: String,
    open: Vec<&'static str>,
    base: usize,
}

impl Xml {
    /// A document, starting with the XML declaration.
    pub(crate) fn document() -> Self {
        Self {
            out: "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n".to_string(),
            open: Vec::new(),
            base: 0,
        }
    }

    /// A fragment whose elements are indented `depth` levels, to be
    /// spliced into a document with [`Xml::splice`].
    pub(crate) fn fragment(depth: usize) -> Self {
        Self {
            out: String::new(),
            open: Vec::new(),
            base: depth,
        }
    }

    pub(crate) fn open(&mut self, tag: &'static str, attrs: Attrs) {
        self.start(tag, attrs);
        self.out.push_str(">\n");
        self.open.push(tag);
    }

    pub(crate) fn leaf(&mut self, tag: &'static str, attrs: Attrs) {
        self.start(tag, attrs);
        self.out.push_str("/>\n");
    }

    pub(crate) fn close(&mut self) {
        if let Some(tag) = self.open.pop() {
            self.indent();
            self.out.push_str("</");
            self.out.push_str(tag);
            self.out.push_str(">\n");
        }
    }

    /// Append a fragment written at this writer's current depth.
    pub(crate) fn splice(&mut self, fragment: Xml) {
        self.out.push_str(&fragment.finish());
    }

    /// The depth the next element is written at.
    pub(crate) fn depth(&self) -> usize {
        self.base + self.open.len()
    }

    /// The text, every open element closed.
    pub(crate) fn finish(mut self) -> String {
        while !self.open.is_empty() {
            self.close();
        }
        self.out
    }

    fn start(&mut self, tag: &str, attrs: Attrs) {
        self.indent();
        self.out.push('<');
        self.out.push_str(tag);
        for (name, value) in attrs {
            self.out.push(' ');
            self.out.push_str(name);
            self.out.push_str("=\"");
            escape_into(&mut self.out, value);
            self.out.push('"');
        }
    }

    fn indent(&mut self) {
        for _ in 0..self.depth() {
            self.out.push_str("  ");
        }
    }
}

/// An attribute value with the five markup characters escaped, white space
/// other than a plain space as a character reference, and every other
/// control character, which XML 1.0 cannot hold, left out.
fn escape_into(out: &mut String, value: &str) {
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' => out.push_str("&#9;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
}

/// A value for an attribute of the schema's qualified-name type: not empty
/// and holding no `:` (a name with one is read as a prefixed name).
pub(crate) fn qualified(value: &str) -> String {
    let name: String = value
        .trim()
        .chars()
        .map(|c| if c == ':' { '_' } else { c })
        .collect();
    if name.is_empty() {
        "_".to_string()
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elements_nest_and_values_are_escaped() {
        let mut xml = Xml::document();
        xml.open("A", &[("n", "x<\"&'>".to_string())]);
        xml.leaf("B", &[("v", "a\tb\u{1}c".to_string())]);
        let text = xml.finish();
        assert_eq!(
            text,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <A n=\"x&lt;&quot;&amp;&apos;&gt;\">\n  <B v=\"a&#9;bc\"/>\n</A>\n"
        );
    }

    #[test]
    fn a_qualified_name_is_never_empty_and_holds_no_colon() {
        assert_eq!(qualified(" a:b "), "a_b");
        assert_eq!(qualified(""), "_");
        assert_eq!(qualified("GND"), "GND");
    }
}
