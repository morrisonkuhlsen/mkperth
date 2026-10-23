//! An XML document as a tree of elements: names without their namespace prefix, the text inside,
//! and the children, built without recursion and with caps on depth and size, since a file is
//! input from anyone.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::P6Error;

/// The deepest nesting and the most elements a file may have.
pub const MAX_DEPTH: usize = 64;
pub const MAX_ELEMENTS: usize = 5_000_000;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Element {
    pub name: String,
    pub text: String,
    pub children: Vec<Element>,
}

impl Element {
    /// The first child named `name`.
    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.name == name)
    }

    /// Every child named `name`.
    pub fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// The trimmed text of the child named `name`, if it has any.
    pub fn text_of(&self, name: &str) -> Option<&str> {
        self.child(name).map(|c| c.text.trim()).filter(|t| !t.is_empty())
    }
}

/// The name after any `prefix:`.
fn local(name: &[u8]) -> String {
    let name = String::from_utf8_lossy(name);
    match name.rsplit_once(':') {
        Some((_, local)) => local.to_string(),
        None => name.into_owned(),
    }
}

/// Parses a whole document into its root element.
pub(crate) fn parse(text: &str) -> Result<Element, P6Error> {
    let mut reader = Reader::from_str(text);
    let bad = |e: &dyn std::fmt::Display| P6Error::Xml(e.to_string());
    let mut stack: Vec<Element> = vec![Element::default()];
    let mut count = 0usize;
    loop {
        match reader.read_event().map_err(|e| bad(&e))? {
            Event::Start(e) => {
                count += 1;
                if stack.len() > MAX_DEPTH || count > MAX_ELEMENTS {
                    return Err(P6Error::TooBig);
                }
                stack.push(Element { name: local(e.name().as_ref()), ..Element::default() });
            }
            Event::Empty(e) => {
                count += 1;
                if count > MAX_ELEMENTS {
                    return Err(P6Error::TooBig);
                }
                let element = Element { name: local(e.name().as_ref()), ..Element::default() };
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(element);
                }
            }
            Event::End(_) => {
                if stack.len() < 2 {
                    return Err(P6Error::Xml(String::from("an end tag without its start")));
                }
                if let Some(done) = stack.pop()
                    && let Some(parent) = stack.last_mut()
                {
                    parent.children.push(done);
                }
            }
            Event::Text(t) => {
                if let Some(e) = stack.last_mut() {
                    e.text.push_str(&t.decode().map_err(|e| bad(&e))?);
                }
            }
            Event::CData(t) => {
                if let Some(e) = stack.last_mut() {
                    e.text.push_str(&t.decode().map_err(|e| bad(&e))?);
                }
            }
            Event::GeneralRef(r) => {
                let resolved = if r.is_char_ref() {
                    r.resolve_char_ref().map_err(|e| bad(&e))?.map(String::from)
                } else {
                    match r.decode().map_err(|e| bad(&e))?.as_ref() {
                        "amp" => Some("&".into()),
                        "lt" => Some("<".into()),
                        "gt" => Some(">".into()),
                        "quot" => Some("\"".into()),
                        "apos" => Some("'".into()),
                        _ => None,
                    }
                };
                if let (Some(text), Some(e)) = (resolved, stack.last_mut()) {
                    e.text.push_str(&text);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if stack.len() != 1 {
        return Err(P6Error::Xml(String::from("the document ends inside an element")));
    }
    stack.pop().and_then(|doc| doc.children.into_iter().next()).ok_or(P6Error::NotP6)
}
