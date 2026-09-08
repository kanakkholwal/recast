//! Text to `Document`, with a position on every element so an error can name a line.
//! Syntax only: well-formed markup with known escapes. Meaning is `validate`'s job, and unknown elements are kept for it.

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::document::{Document, Node};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("{at}: {message}")]
    Syntax { at: Position, message: String },
    #[error("the document is empty")]
    Empty,
    #[error("{at}: the root element is <{found}>, expected <recast>")]
    WrongRoot { at: Position, found: String },
    #[error("{at}: text is only allowed inside a text-bearing element, found it in <{parent}>")]
    StrayText { at: Position, parent: String },
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}, column {}", self.line, self.column)
    }
}

/// # Errors On malformed markup, a missing or wrong root, or text where none belongs.
pub fn parse(source: &str) -> Result<Document, ParseError> {
    let mut reader = Reader::from_str(source);
    reader.config_mut().trim_text(true);
    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;

    loop {
        let offset = reader.buffer_position() as usize;
        let at = position_of(source, skip_ws(source, offset));
        let event = reader.read_event().map_err(|e| ParseError::Syntax {
            at,
            message: e.to_string(),
        })?;
        match event {
            Event::Start(start) => stack.push(node_from(&start, at)?),
            Event::Empty(start) => {
                let node = node_from(&start, at)?;
                close(&mut stack, &mut root, node, at)?;
            }
            Event::End(_) => {
                let Some(node) = stack.pop() else {
                    return Err(ParseError::Syntax {
                        at,
                        message: "unexpected closing tag".into(),
                    });
                };
                close(&mut stack, &mut root, node, at)?;
            }
            Event::Text(text) => {
                let text = text.unescape().map_err(|e| ParseError::Syntax {
                    at,
                    message: e.to_string(),
                })?;
                let Some(parent) = stack.last_mut() else {
                    return Err(ParseError::StrayText {
                        at,
                        parent: "document".into(),
                    });
                };
                if !crate::schema::has_text(&parent.kind) {
                    return Err(ParseError::StrayText {
                        at,
                        parent: parent.kind.clone(),
                    });
                }
                parent.text = Some(text.trim().to_owned());
            }
            Event::Eof => break,
            _ => {}
        }
    }

    let root = root.ok_or(ParseError::Empty)?;
    if root.kind != "recast" {
        return Err(ParseError::WrongRoot {
            at: root.at.unwrap_or_default(),
            found: root.kind,
        });
    }
    Ok(Document { root })
}

fn close(
    stack: &mut [Node],
    root: &mut Option<Node>,
    node: Node,
    at: Position,
) -> Result<(), ParseError> {
    match stack.last_mut() {
        Some(parent) => parent.children.push(node),
        None if root.is_none() => *root = Some(node),
        None => {
            return Err(ParseError::Syntax {
                at,
                message: "a second root element".into(),
            });
        }
    }
    Ok(())
}

fn node_from(start: &quick_xml::events::BytesStart<'_>, at: Position) -> Result<Node, ParseError> {
    let kind = String::from_utf8_lossy(start.name().as_ref()).into_owned();
    let mut node = Node::new(&kind);
    node.at = Some(at);
    for attr in start.attributes() {
        let attr = attr.map_err(|e| ParseError::Syntax {
            at,
            message: e.to_string(),
        })?;
        let name = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
        let value = attr.unescape_value().map_err(|e| ParseError::Syntax {
            at,
            message: e.to_string(),
        })?;
        if node
            .attrs
            .insert(name.clone(), value.into_owned())
            .is_some()
        {
            return Err(ParseError::Syntax {
                at,
                message: format!("attribute '{name}' given twice"),
            });
        }
    }
    Ok(node)
}

/// The reader sits at the end of the previous event, which is before the whitespace leading to the next tag.
fn skip_ws(source: &str, offset: usize) -> usize {
    let offset = offset.min(source.len());
    offset + source[offset..].len() - source[offset..].trim_start().len()
}

fn position_of(source: &str, offset: usize) -> Position {
    let offset = offset.min(source.len());
    let before = &source[..offset];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map_or(0, str::len) + 1;
    Position { line, column }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_small_document_parses_into_a_tree_with_positions() {
        let src = "<recast v=\"3\">\n  <screen id=\"scr\">\n    <zoom id=\"z1\" at=\"4.500\"/>\n  </screen>\n</recast>";
        let doc = parse(src).unwrap();
        assert_eq!(doc.version(), Some(3));
        let zoom = doc.find("z1").unwrap();
        assert_eq!(zoom.attr("at"), Some("4.500"));
        assert_eq!(zoom.at, Some(Position { line: 3, column: 5 }));
    }

    #[test]
    fn text_lands_only_on_text_bearing_elements() {
        let ok = parse("<recast v=\"3\"><annotations><text id=\"t1\">Hello &amp; welcome</text></annotations></recast>").unwrap();
        assert_eq!(
            ok.find("t1").unwrap().text.as_deref(),
            Some("Hello & welcome")
        );
        let err = parse("<recast v=\"3\"><screen>stray</screen></recast>").unwrap_err();
        assert!(matches!(err, ParseError::StrayText { .. }));
    }

    #[test]
    fn malformed_markup_names_the_line() {
        let err = parse("<recast v=\"3\">\n<screen>\n</recast>").unwrap_err();
        assert!(
            matches!(err, ParseError::Syntax { at, .. } if at.line == 3),
            "{err}"
        );
    }

    #[test]
    fn the_root_must_be_recast_and_present() {
        assert!(matches!(
            parse("<scene/>").unwrap_err(),
            ParseError::WrongRoot { .. }
        ));
        assert_eq!(parse("   ").unwrap_err(), ParseError::Empty);
    }

    #[test]
    fn a_repeated_attribute_is_a_syntax_error() {
        let err = parse("<recast v=\"3\" v=\"4\"/>").unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn comments_are_ignored() {
        let doc = parse("<!-- note --><recast v=\"3\"><!-- inner --></recast>").unwrap();
        assert!(doc.root.children.is_empty());
    }
}
