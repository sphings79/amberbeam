//! What the two XML formats have in common.
//!
//! Both are shallow: elements with text in them, nested a level or two. Neither
//! needs namespaces, entities beyond the five, or attributes past one or two —
//! so what lives here is the walk over the events and a Base64 decoder, and the
//! two readers say what to do with each element.

use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion};

/// One element, as these formats use them: a name, its attributes, and the text
/// directly inside it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    pub text: String,
    pub attributes: Vec<(String, String)>,
}

impl Element {
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// What the walk reports, in the order it happens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// An element began. Its text is whatever came before the first child.
    Open(Element),
    /// An element ended, by name.
    Close(String),
}

/// Walks the document, reporting every element as it opens and closes.
///
/// Text is gathered onto the element that is open at the time, which is what
/// makes FileZilla's folders readable: a folder's name is loose text inside the
/// `Folder` element, sitting in front of the servers it holds.
pub fn walk(text: &str) -> Vec<Step> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(false);

    let mut steps: Vec<Step> = Vec::new();
    let mut open: Vec<usize> = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                let element = element_of(start.name().as_ref(), start.attributes());
                open.push(steps.len());
                steps.push(Step::Open(element));
            }
            Ok(Event::Empty(start)) => {
                let element = element_of(start.name().as_ref(), start.attributes());
                let name = element.name.clone();
                steps.push(Step::Open(element));
                steps.push(Step::Close(name));
            }
            Ok(Event::Text(chunk)) => {
                let Some(&index) = open.last() else { continue };
                if let Some(Step::Open(element)) = steps.get_mut(index) {
                    element.text.push_str(&chunk);
                }
            }
            // An entity arrives as an event of its own, splitting the text
            // around it. Both kinds matter here: `&amp;` for the ampersand in a
            // server's name, `&#246;` for the umlaut in the customer's.
            Ok(Event::GeneralRef(reference)) => {
                let Some(&index) = open.last() else { continue };
                let resolved = match reference.resolve_char_ref() {
                    Ok(Some(character)) => Some(character),
                    Ok(None) => named_entity(&reference),
                    Err(_) => None,
                };
                if let (Some(character), Some(Step::Open(element))) =
                    (resolved, steps.get_mut(index))
                {
                    element.text.push(character);
                }
            }
            Ok(Event::End(end)) => {
                open.pop();
                steps.push(Step::Close(end.name().as_ref().to_owned()));
            }
            Ok(Event::Eof) => break,
            // A document that stops making sense half way through still gave up
            // whatever came before, and half a list beats none.
            Err(_) => break,
            _ => {}
        }
    }
    steps
}

/// The five entities XML defines. Anything else in these files would be a
/// document-defined entity, which neither format uses.
fn named_entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => None,
    }
}

fn element_of(name: &str, attributes: quick_xml::events::attributes::Attributes<'_>) -> Element {
    Element {
        name: name.to_owned(),
        text: String::new(),
        attributes: attributes
            .flatten()
            .map(|attribute| {
                (
                    attribute.key.as_ref().to_owned(),
                    attribute
                        .normalized_value(XmlVersion::Implicit1_0)
                        .map(|value| value.into_owned())
                        .unwrap_or_default(),
                )
            })
            .collect(),
    }
}

/// Base64, the decoding half.
///
/// Twenty lines against a dependency, for a job with no variants worth
/// supporting: these files use the standard alphabet and pad with `=`.
pub fn from_base64(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut buffer = 0_u32;
    let mut bits = 0_u32;

    for c in text.chars() {
        if c.is_whitespace() || c == '=' {
            continue;
        }
        let value = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            _ => return None,
        };
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_lands_on_the_element_it_was_written_in() {
        // The shape FileZilla uses for folders: loose text first, then the
        // servers it holds.
        let steps =
            walk("<Folder expanded=\"1\">Kunden<Server><Host>a.example</Host></Server></Folder>");
        let Step::Open(folder) = &steps[0] else {
            panic!("expected an element")
        };
        assert_eq!(folder.name, "Folder");
        assert_eq!(folder.text, "Kunden");
        assert_eq!(folder.attribute("expanded"), Some("1"));

        let names: Vec<&str> = steps
            .iter()
            .filter_map(|step| match step {
                Step::Open(element) => Some(element.name.as_str()),
                Step::Close(_) => None,
            })
            .collect();
        assert_eq!(names, vec!["Folder", "Server", "Host"]);
    }

    #[test]
    fn an_empty_element_opens_and_closes() {
        let steps = walk("<Server><LocalDir /></Server>");
        assert_eq!(steps.len(), 4);
        assert_eq!(steps[2], Step::Close("LocalDir".into()));
    }

    #[test]
    fn the_five_entities_come_back_as_characters() {
        let steps = walk("<Name>Gr&#246;&#223;e &amp; Ma&#223;</Name>");
        let Step::Open(element) = &steps[0] else {
            panic!("expected an element")
        };
        assert_eq!(element.text, "Größe & Maß");
    }

    #[test]
    fn a_document_that_stops_making_sense_keeps_what_came_before() {
        let steps = walk("<Servers><Server><Host>a.example</Host></Server><Serv");
        assert!(steps
            .iter()
            .any(|step| matches!(step, Step::Open(e) if e.name == "Host")));
    }

    #[test]
    fn base64_comes_back() {
        assert_eq!(
            from_base64("dGFubmVuYmF1bQ==").as_deref(),
            Some("tannenbaum".as_bytes())
        );
        assert_eq!(from_base64("").as_deref(), Some(&[][..]));
        assert_eq!(from_base64("!!!"), None);
    }
}
