use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Id(String);

impl From<String> for Id {
    fn from(id: String) -> Self {
        Id(id)
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
