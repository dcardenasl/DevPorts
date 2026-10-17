//! Where each key of a YAML document is, by path.
//!
//! Validation messages are only useful if they say which line to fix. The typed
//! deserialisation does not keep positions (and loses them for unknown keys), so
//! the document is also walked once as a stream of parser events, recording the
//! line of every mapping key under its path (`services` > `api` > `port`).

use std::collections::BTreeMap;

use serde_saphyr::granit_parser::{Event, Parser};

/// The line of every key, by path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyIndex {
    lines: BTreeMap<Vec<String>, u64>,
}

impl KeyIndex {
    /// The 1-based line of the key at `path`, if there is one.
    #[must_use]
    pub fn line(&self, path: &[&str]) -> Option<u64> {
        let key: Vec<String> = path.iter().map(|part| (*part).to_owned()).collect();
        self.lines.get(&key).copied()
    }

    /// Every key whose parent is `parent`, with its line, in file order of path.
    #[must_use]
    pub fn children(&self, parent: &[&str]) -> Vec<(String, u64)> {
        self.lines
            .iter()
            .filter(|(path, _)| {
                path.len() == parent.len().saturating_add(1)
                    && path.iter().zip(parent).all(|(a, b)| a == b)
            })
            .filter_map(|(path, line)| path.last().map(|name| (name.clone(), *line)))
            .collect()
    }
}

enum Frame {
    /// A mapping: the keys on the path to it, and whether the next scalar is a key.
    Map { expecting_key: bool },
    /// A sequence: its scalars are values, not keys.
    Seq,
}

/// Indexes the keys of `text`. A document that does not parse yields an empty
/// index; the caller reports the parse error itself.
#[must_use]
pub fn index(text: &str) -> KeyIndex {
    let mut lines = BTreeMap::new();
    let mut frames: Vec<Frame> = Vec::new();
    // The key under which each open mapping or sequence hangs, root first.
    let mut path: Vec<String> = Vec::new();
    let mut pending_key: Option<String> = None;

    for event in Parser::new_from_str(text) {
        let Ok((event, span)) = event else {
            return KeyIndex::default();
        };
        match event {
            Event::MappingStart(..) => {
                // A mapping that is a value hangs under the key just read.
                if let Some(key) = pending_key.take() {
                    path.push(key);
                } else if matches!(frames.last(), Some(Frame::Seq)) {
                    path.push(String::from("[]"));
                }
                frames.push(Frame::Map {
                    expecting_key: true,
                });
            }
            Event::SequenceStart(..) => {
                if let Some(key) = pending_key.take() {
                    path.push(key);
                } else if matches!(frames.last(), Some(Frame::Seq)) {
                    path.push(String::from("[]"));
                }
                frames.push(Frame::Seq);
            }
            Event::MappingEnd | Event::SequenceEnd => {
                frames.pop();
                path.pop();
                if let Some(Frame::Map { expecting_key }) = frames.last_mut() {
                    *expecting_key = true;
                }
            }
            Event::Scalar(value, ..) => match frames.last_mut() {
                Some(Frame::Map { expecting_key }) if *expecting_key => {
                    let mut full = path.clone();
                    full.push(value.to_string());
                    lines.insert(full, u64::try_from(span.start.line()).unwrap_or(0));
                    pending_key = Some(value.to_string());
                    *expecting_key = false;
                }
                Some(Frame::Map { expecting_key }) => {
                    pending_key = None;
                    *expecting_key = true;
                }
                _ => {}
            },
            _ => {}
        }
    }
    KeyIndex { lines }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "\
version: 1
project: shop
uses: [mysql, redis]
services:
  api:
    cwd: ./api
    port: 8080
  web:
    docker: nginx
    extra:
      nested: true
";

    #[test]
    fn every_key_has_its_line_by_path() {
        let keys = index(DOC);
        assert_eq!(keys.line(&["version"]), Some(1));
        assert_eq!(keys.line(&["project"]), Some(2));
        assert_eq!(keys.line(&["services"]), Some(4));
        assert_eq!(keys.line(&["services", "api"]), Some(5));
        assert_eq!(keys.line(&["services", "api", "port"]), Some(7));
        assert_eq!(keys.line(&["services", "web", "docker"]), Some(9));
        assert_eq!(keys.line(&["services", "web", "extra", "nested"]), Some(11));
        assert_eq!(keys.line(&["services", "api", "missing"]), None);
    }

    #[test]
    fn children_lists_the_direct_keys_of_a_path() {
        let keys = index(DOC);
        let names: Vec<String> = keys
            .children(&["services"])
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        assert_eq!(names, ["api", "web"]);
        let top: Vec<String> = keys
            .children(&[])
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        assert_eq!(top, ["project", "services", "uses", "version"]);
    }

    #[test]
    fn scalars_in_a_sequence_are_not_keys() {
        let keys = index("uses: [a, b]\nname: x\n");
        assert_eq!(keys.line(&["uses"]), Some(1));
        assert_eq!(keys.line(&["a"]), None);
        assert_eq!(keys.line(&["name"]), Some(2));
    }

    #[test]
    fn a_broken_document_gives_an_empty_index() {
        assert_eq!(index("a: [unclosed\n"), KeyIndex::default());
    }
}
