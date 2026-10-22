use super::parser::{MarkupParser, Run};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Markup(Vec<Run>);

impl Markup {
    pub fn parse(body: &str) -> Self {
        Self(MarkupParser::new(body).parse())
    }

    pub fn into_runs(self) -> Vec<Run> {
        self.0
    }
}
