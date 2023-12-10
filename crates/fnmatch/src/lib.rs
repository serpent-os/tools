// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    str::FromStr,
};

use regex::Regex;
use thiserror::Error;
#[derive(Debug)]
enum Fragment {
    /// `?`
    MatchOne,

    /// `*`
    MatchAny,

    /// `\`
    BackSlash,

    /// `.`
    Dot,

    /// `/
    ForwardSlash,

    /// Normal text.
    Text(String),

    /// Group: Name to fragment mapping
    Group(String, Vec<Fragment>),
}

#[derive(Clone)]
struct StringWalker<'a> {
    data: &'a str,
    index: usize,
    length: usize,
}

impl<'a> Iterator for StringWalker<'a> {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.length {
            None
        } else {
            self.index += 1;
            self.data
                .get(self.index - 1..self.index)
                .and_then(|s| s.chars().nth(0))
        }
    }
}

impl<'a> StringWalker<'a> {
    /// Return a new StringWalker
    pub fn new(data: &'a str) -> Self {
        Self {
            data,
            index: 0,
            length: data.len(),
        }
    }

    pub fn eat(&mut self, much: usize) {
        self.index += much
    }

    /// Find next occurance of the character, and substring up to it
    pub fn substring_to(&self, c: char) -> Option<&'a str> {
        // Clone ourselves and search that iterator.
        let walker = self.clone();
        for (idx, t) in walker.enumerate() {
            if t == c {
                return self.data.get(self.index..self.index + idx);
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct Pattern {
    pub pattern: String,
    regex: Regex,
    groups: Vec<String>,
}

#[derive(Debug)]
pub struct Match {
    pub path: String,
    pub variables: HashMap<String, String>,
}

impl Pattern {
    pub fn match_path(&self, path: &str) -> Option<Match> {
        match self.regex.captures(path) {
            Some(m) => {
                let kv = self
                    .groups
                    .iter()
                    .map(|k| (k.clone(), m.name(k).unwrap().as_str().to_string()));
                Some(Match {
                    path: path.into(),
                    variables: kv.collect(),
                })
            }
            None => None,
        }
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("malformed: {0}")]
    String(#[from] Infallible),

    #[error("malformed group")]
    Group,

    #[error("invalid regex: {0}")]
    Regex(#[from] regex::Error),
}

fn fragments_from_string(s: &str) -> Result<Vec<Fragment>, Error> {
    let mut walker = StringWalker::new(s);
    let mut builder = vec![];
    let mut text = String::new();
    while let Some(ch) = walker.next() {
        let next_token = match ch {
            '?' => Some(Fragment::MatchOne),
            '*' => Some(Fragment::MatchAny),
            '\\' => Some(Fragment::BackSlash),
            '/' => Some(Fragment::ForwardSlash),
            '.' => Some(Fragment::Dot),
            '(' => {
                if let Some(end) = walker.substring_to(')') {
                    walker.eat(end.len() + 1);

                    let splits = end.split(':').collect::<Vec<_>>();
                    if splits.len() != 2 {
                        return Err(Error::Group);
                    }
                    let key = splits.first().ok_or(Error::Group)?;
                    let value = splits.get(1).ok_or(Error::Group)?;

                    let subpattern = fragments_from_string(value)?;
                    builder.push(Fragment::Group(String::from_str(key)?, subpattern));
                } else {
                    return Err(Error::Group);
                }
                None
            }
            ')' => None,
            _ => {
                text.push(ch);
                None
            }
        };

        if let Some(token) = next_token {
            if !text.is_empty() {
                builder.push(Fragment::Text(text.clone()));
                text.clear();
            }
            builder.push(token)
        }
    }

    if !text.is_empty() {
        builder.push(Fragment::Text(text.clone()));
    }

    Ok(builder)
}

fn fragment_to_regex_str(fragment: &Fragment) -> (String, Vec<String>) {
    let mut groups = vec![];
    let string = match fragment {
        Fragment::MatchOne => ".".into(),
        Fragment::MatchAny => "[^\\/]*".into(),
        Fragment::BackSlash => "\\".into(),
        Fragment::ForwardSlash => "\\/".into(),
        Fragment::Dot => "\\.".into(),
        Fragment::Text(t) => t.clone(),
        Fragment::Group(id, elements) => {
            let elements = elements
                .iter()
                .map(|m| {
                    let (s, g) = fragment_to_regex_str(m);
                    groups.extend(g);
                    s
                })
                .collect::<String>();
            groups.push(id.clone());
            format!("(?<{id}>{elements})")
        }
    };
    (string, groups)
}

impl FromStr for Pattern {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let fragments = fragments_from_string(s)?;
        let mut groups = HashSet::new();

        let compiled = fragments
            .iter()
            .map(|m| {
                let (s, g) = fragment_to_regex_str(m);
                groups.extend(g);
                s
            })
            .collect::<String>();

        Ok(Self {
            pattern: s.into(),
            regex: Regex::new(&compiled)?,
            groups: groups.into_iter().collect(),
        })
    }
}

#[cfg(test)]
pub mod path_tests {
    use super::Pattern;

    /// test me
    #[test]
    fn test_pattern() {
        let k = "/usr/lib/modules/(version:*)/modules.symbols"
            .parse::<Pattern>()
            .unwrap();

        let good = k.match_path("/usr/lib/modules/6.2.6/modules.symbols");
        assert!(good.is_some());
        let m = good.unwrap();
        assert_eq!(m.path, "/usr/lib/modules/6.2.6/modules.symbols");
        let version = m.variables.get("version");
        assert!(version.is_some());
        assert_eq!(version.unwrap(), "6.2.6");

        let bad = k.match_path("/usr/lib/modules/6.2.6/l/modules.symbols");
        assert!(bad.is_none());
    }
}
