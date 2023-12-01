// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0
#![allow(clippy::map_collect_result_unit)]

use std::collections::{HashMap, HashSet};

use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{alpha1, anychar, char, digit1},
    combinator::{eof, iterator, map, peek, recognize, value},
    multi::{many1, many_till},
    sequence::{delimited, preceded, terminated},
};
use thiserror::Error;

use crate::{macros::Action, Macros};

#[derive(Default)]
pub struct Parser {
    actions: HashMap<String, Action>,
    definitions: HashMap<String, String>,
}

impl Parser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_action(&mut self, identifier: impl ToString, action: Action) {
        self.actions.insert(identifier.to_string(), action);
    }

    pub fn add_definition(&mut self, identifier: impl ToString, definition: impl ToString) {
        self.definitions
            .insert(identifier.to_string(), definition.to_string());
    }

    pub fn add_macros(&mut self, macros: Macros) {
        macros.actions.into_iter().for_each(|kv| {
            self.add_action(kv.key, kv.value);
        });
        macros.definitions.into_iter().for_each(|kv| {
            self.add_definition(kv.key, kv.value);
        });
    }

    pub fn parse(&self, input: &str) -> Result<Script, Error> {
        parse(input, &self.actions, &self.definitions)
    }
}

pub struct Script {
    pub content: String,
    pub dependencies: Vec<String>,
}

fn parse(
    input: &str,
    actions: &HashMap<String, Action>,
    definitions: &HashMap<String, String>,
) -> Result<Script, Error> {
    let mut content = String::new();
    let mut dependencies = HashSet::new();

    tokens(input, |token| {
        match token {
            Token::Action(identifier) => {
                let action = actions
                    .get(identifier)
                    .ok_or(Error::UnknownAction(identifier.to_string()))?;
                dependencies.extend(action.dependencies.clone());

                let script = parse(&action.command, actions, definitions)?;

                content.push_str(&script.content);
                dependencies.extend(script.dependencies);
            }
            Token::Definition(identifier) => {
                let definition = definitions
                    .get(identifier)
                    .ok_or(Error::UnknownDefinition(identifier.to_string()))?;

                let script = parse(definition, actions, definitions)?;

                content.push_str(&script.content);
                dependencies.extend(script.dependencies);
            }
            Token::Plain(plain) => content.push_str(plain),
        }
        Ok(())
    })?;

    Ok(Script {
        content,
        dependencies: dependencies.into_iter().collect(),
    })
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
    use crate::macros::Action;

    #[test]
    fn parse_script() {
        let input =
            "%patch %%escaped %{ %(pkgdir)/0001-deps-analysis-elves-In-absence-of-soname.-make-one-u.patch";

        let mut parser = Parser::new();
        parser.add_action(
            "patch",
            Action {
                command: "patch -v %(nested_flag)".into(),
                dependencies: vec!["patch".into()],
            },
        );

        for (id, definition) in [
            ("nested_flag", "--args=%(nested_arg),b,c"),
            ("nested_arg", "a"),
            ("pkgdir", "%(root)/pkg"),
            ("root", "/mason"),
        ] {
            parser.add_definition(id, definition);
        }

        let script = parser.parse(input).unwrap();

        assert_eq!(script.content, "patch -v --args=a,b,c %escaped %{ /mason/pkg/0001-deps-analysis-elves-In-absence-of-soname.-make-one-u.patch".to_string());
        assert_eq!(script.dependencies, vec!["patch".to_string()])
    }
}
