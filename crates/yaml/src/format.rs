// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{
    borrow::Cow,
    fmt::{self, Write},
};

use itertools::Itertools;
use serde::Serialize;
use serde_yaml::Value;
use thiserror::Error;

/// Spaces per tab (indent)
const TAB_SIZE: usize = 4;
/// Addtl. space for sequence items due to `- `
const SEQ_SPACE: usize = 2;

pub fn format<T: Serialize>(value: &T) -> Result<String, Error> {
    let mut f = String::new();

    let value = serde_yaml::to_value(value)?;

    nested_format(&mut f, &value, 0, false)?;

    Ok(f)
}

fn nested_format(
    f: &mut String,
    value: &Value,
    level: usize,
    is_seq_item: bool,
) -> Result<(), Error> {
    match value {
        Value::Null => writeln!(f, "{}~", indent(level, is_seq_item, 0))?,
        Value::Bool(bool) => writeln!(f, "{}{bool}", indent(level, is_seq_item, 0))?,
        Value::Number(number) => writeln!(f, "{}{number}", indent(level, is_seq_item, 0))?,
        Value::String(string) => writeln!(f, "{}{string}", indent(level, is_seq_item, 0))?,
        // Strip the tag, we don't support it
        Value::Tagged(tagged) => nested_format(f, &tagged.value, level, is_seq_item)?,
        Value::Sequence(sequence) => {
            for (i, value) in sequence.iter().enumerate() {
                write!(f, "{}- ", indent(level, is_seq_item, i))?;

                match formatted_scalar(value) {
                    Some(scalar) => writeln!(f, "{}", formatted_string(&scalar, level + 1))?,
                    None => {
                        nested_format(f, value, level, true)?;
                    }
                }
            }
        }
        Value::Mapping(mapping) => {
            let entries = mapping
                .iter()
                .map(|(key, value)| {
                    formatted_scalar(key)
                        .ok_or(Error::NonScalarKey)
                        .map(|key| (key, value))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let max_key = entries
                .iter()
                .map(|(key, _)| key.len())
                .max()
                .unwrap_or_default();

            for (i, (key, value)) in entries.into_iter().enumerate() {
                let indent = indent(level, is_seq_item, i);

                match formatted_scalar(value) {
                    Some(scalar) => {
                        writeln!(
                            f,
                            "{indent}{:<max_key$} : {}",
                            key.trim_end(),
                            formatted_string(&scalar, level + 1)
                        )?;
                    }
                    None => {
                        writeln!(f, "{indent}{:<max_key$} :", key.trim_end())?;

                        nested_format(f, value, level + 1, false)?;
                    }
                }
            }
        }
    }

    Ok(())
}

fn formatted_scalar(value: &Value) -> Option<String> {
    match value {
        Value::Null => Some("~".to_string()),
        Value::Bool(bool) => Some(bool.to_string()),
        Value::Number(number) => Some(number.to_string()),
        Value::String(string) => Some(string.to_string()),
        // Strip the tag, we don't support it
        Value::Tagged(tagged) => formatted_scalar(&tagged.value),
        Value::Sequence(_) | Value::Mapping(_) => None,
    }
}

fn formatted_string(s: &str, level: usize) -> Cow<str> {
    let num_newlines = s.chars().filter(|c| *c == '\n').count();

    if num_newlines == 0 {
        Cow::Borrowed(s)
    } else {
        let indent = indent(level, false, 0);
        let indented = s.lines().map(|line| format!("{indent}{line}")).join("\n");
        Cow::Owned(format!("|\n{indented}"))
    }
}

fn indent(level: usize, is_seq_item: bool, i: usize) -> String {
    if is_seq_item && i == 0 {
        String::new()
    } else {
        let mut num_spaces = TAB_SIZE * level;
        if is_seq_item {
            num_spaces += SEQ_SPACE;
        }
        " ".repeat(num_spaces)
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("non scalar value used for map key")]
    NonScalarKey,
    #[error(transparent)]
    Serialize(#[from] serde_yaml::Error),
    #[error(transparent)]
    Format(#[from] fmt::Error),
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_format() {
        let raw = r#"name : value
version : asdfasdf
description: |
    asdfasdf
upstreams : 
  - https://asdf.com: 12341234"#;
        let expected = r#"name        : value
version     : asdfasdf
description : |
    asdfasdf
upstreams   :
    - https://asdf.com : 12341234
"#;

        let map = serde_yaml::from_str::<serde_yaml::Mapping>(raw).unwrap();
        let formatted = format(&map).unwrap();

        assert_eq!(formatted, expected);
    }

    #[test]
    fn roundtrip() {
        let test = include_str!("../../../test/boulder-stone.yml");
        let map = serde_yaml::from_str::<serde_yaml::Mapping>(test).unwrap();
        let formatted = format(&map).unwrap();
        let rt_map = serde_yaml::from_str::<serde_yaml::Mapping>(&formatted).unwrap();

        assert_eq!(map, rt_map);
    }
}
