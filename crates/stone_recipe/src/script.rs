// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0
#![allow(clippy::map_collect_result_unit)]

use std::collections::HashMap;

use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{alpha1, anychar, char, digit1},
    combinator::{eof, iterator, map, peek, recognize, value},
    multi::{many1, many_till},
    sequence::{delimited, preceded, terminated},
};
use thiserror::Error;

pub fn parse(input: &str, macros: &HashMap<String, String>) -> Result<String, Error> {
    let mut output = String::new();

    tokens(input, |token| {
        match token {
            Token::Action(action) => {
                let replacement = macros
                    .get(action)
                    .ok_or(Error::UnknownAction(action.to_string()))?;

                output.push_str(&parse(replacement, macros)?);
            }
            Token::Definition(definition) => {
                let replacement = macros
                    .get(definition)
                    .ok_or(Error::UnknownDefinition(definition.to_string()))?;

                output.push_str(&parse(replacement, macros)?);
            }
            Token::Plain(plain) => output.push_str(plain),
        }
        Ok(())
    })?;

    Ok(output)
}

#[derive(Debug)]
enum Token<'a> {
    Action(&'a str),
    Definition(&'a str),
    Plain(&'a str),
}

fn tokens(input: &str, f: impl FnMut(Token) -> Result<(), Error>) -> Result<(), Error> {
    // A-Za-z0-9_
    let identifier = |input| recognize(many1(alt((alpha1, digit1, tag("_")))))(input);
    // %identifier
    let action = |input| preceded(char('%'), identifier)(input);
    // %(identifier)
    let definition =
        |input| preceded(char('%'), delimited(char('('), identifier, char(')')))(input);
    // action or definition
    let macro_ = alt((action, definition));
    // %% -> %
    let escaped = |input| preceded(char('%'), value("%", char('%')))(input);
    // Escaped or any char until escape, next macro or EOF
    let plain = alt((
        escaped,
        recognize(many_till(anychar, peek(alt((escaped, macro_))))),
        recognize(terminated(many1(anychar), eof)),
    ));

    let token = alt((
        map(action, Token::Action),
        map(definition, Token::Definition),
        map(plain, Token::Plain),
    ));

    let mut iter = iterator(input, token);

    iter.map(f).collect::<Result<(), Error>>()?;

    iter.finish().map_err(convert_error)?;

    Ok(())
}

fn convert_error(
    err: nom::Err<(&str, nom::error::ErrorKind)>,
) -> nom::Err<nom::error::Error<String>> {
    err.to_owned().map(|(i, e)| nom::error::Error::new(i, e))
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("unknown action macro: %{0}")]
    UnknownAction(String),
    #[error("unknown definition macro: %({0})")]
    UnknownDefinition(String),
    #[error("parse script")]
    Parser(#[from] nom::Err<nom::error::Error<String>>),
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_script() {
        let input =
            "%patch %%escaped %{ %(pkgdir)/0001-deps-analysis-elves-In-absence-of-soname.-make-one-u.patch";

        let macros = HashMap::from_iter([
            ("patch".into(), "patch -v %(nested_flag)".into()),
            ("nested_flag".into(), "--args=%(nested_arg),b,c".into()),
            ("nested_arg".into(), "a".into()),
            ("pkgdir".into(), "%(root)/pkg".into()),
            ("root".into(), "/mason".into()),
        ]);

        let parsed = parse(input, &macros).unwrap();

        assert_eq!(parsed, "patch -v --args=a,b,c %escaped %{ /mason/pkg/0001-deps-analysis-elves-In-absence-of-soname.-make-one-u.patch".to_string());
    }
}
