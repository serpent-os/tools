// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use glob::Pattern;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Rule {
    pub pattern: String,
    pub target: String,
}

impl Rule {
    pub fn matches(&self, path: &str) -> bool {
        self.pattern == path
            || path.starts_with(&self.pattern)
            || Pattern::new(&self.pattern)
                .map(|pattern| pattern.matches(path))
                .unwrap_or_default()
    }
}

#[derive(Debug, Default)]
pub struct Matcher {
    /// Rules stored in order of
    /// ascending priority
    rules: Vec<Rule>,
}

impl Matcher {
    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    pub fn matching_target(&self, path: &str) -> Option<&str> {
        // Rev = check highest priority rules first
        self.rules
            .iter()
            .rev()
            .find_map(|rule| rule.matches(path).then_some(rule.target.as_str()))
    }
}
