use mecha_wayland::{mechanix_widgets::TextDecoration, prelude::FontWeight};

use crate::markup::lexer::Token::{self, Close, Open, Text};
use crate::markup::lexer::{Lexer, Tag};

pub struct MarkupParser<'parser> {
    lexer: Lexer<'parser>,
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct Run {
    pub words: String,
    pub style: Option<Style>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    pub weight: FontWeight,
    pub slant: Slant,
    pub decoration: TextDecoration,
    pub link: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Slant {
    Upright,
    Italic,
}

impl<'parser> MarkupParser<'parser> {
    pub fn new(body: &'parser str) -> Self {
        Self {
            lexer: Lexer::new(body),
        }
    }

    /// Runs of text with the style of the tags around them. Only matched
    /// pairs style text; an unclosed open or a stray close is ignored, and
    /// its text stays plain.
    pub fn parse(self) -> Vec<Run> {
        return runs(self.lexer.collect());
    }
}

fn runs(tokens: Vec<Token>) -> Vec<Run> {
    let mut stack: Vec<&Tag> = Vec::new();
    let marker_indices = mark_valid_tag_pairs(&tokens);
    let mut runs: Vec<Run> = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        match token {
            Open(_) | Close(_) if marker_indices[index] == false => continue,
            Open(tag) => {
                stack.push(tag);
            }
            Close(_) => {
                stack.pop();
            }
            Text(text) => {
                // TODO: decode &amp; &lt; ...
                let style = style(&stack);
                match runs.last_mut() {
                    Some(last) if last.style == style => last.words.push_str(text),
                    _ => runs.push(Run {
                        words: text.to_string(),
                        style,
                    }),
                }
            }
        }
    }
    return runs;
}

fn style(stack: &[&Tag]) -> Option<Style> {
    if stack.is_empty() {
        return None;
    }
    let has = |tag: Tag| stack.iter().any(|&open| *open == tag);
    Some(Style {
        weight: if has(Tag::Bold) {
            FontWeight::Bold
        } else {
            FontWeight::Regular
        },
        slant: if has(Tag::Italic) {
            Slant::Italic
        } else {
            Slant::Upright
        },
        decoration: if has(Tag::Underline) {
            TextDecoration::Underline
        } else {
            TextDecoration::None
        },
        link: None,
    })
}

fn mark_valid_tag_pairs(tokens: &Vec<Token>) -> Vec<bool> {
    let mut marker: Vec<bool> = vec![false; tokens.len()];
    let mut indices: Vec<(&Tag, usize)> = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        match token {
            Open(tag) => {
                indices.push((tag, i));
            }
            Close(tag) => {
                if let Some(&(open_tag, open_index)) = indices.last()
                    && tag == open_tag
                {
                    indices.pop();
                    marker[open_index] = true;
                    marker[i] = true;
                }
            }
            _ => continue,
        }
    }
    return marker;
}

#[cfg(test)]
fn parse(tokens: Vec<Token>) -> Vec<Run> {
    runs(tokens)
}

#[cfg(test)]
fn run(words: &str, tags: &[Tag]) -> Run {
    let stack: Vec<&Tag> = tags.iter().collect();
    Run {
        words: words.into(),
        style: style(&stack),
    }
}

#[test]
fn matched_tags_are_styled() {
    // <b> <i> hello </i> world </b>
    assert_eq!(
        parse(vec![
            Open(Tag::Bold),
            Text(" "),
            Open(Tag::Italic),
            Text(" hello "),
            Close(Tag::Italic),
            Text(" world "),
            Close(Tag::Bold),
        ]),
        vec![
            run(" ", &[Tag::Bold]),
            run(" hello ", &[Tag::Bold, Tag::Italic]),
            run(" world ", &[Tag::Bold]),
        ]
    );
}

#[test]
fn unclosed_tag_is_plain() {
    // <b>Pritam:</b> <i>see you at the sta
    assert_eq!(
        parse(vec![
            Open(Tag::Bold),
            Text("Pritam:"),
            Close(Tag::Bold),
            Text(" "),
            Open(Tag::Italic),
            Text("see you at the sta"),
        ]),
        vec![
            run("Pritam:", &[Tag::Bold]),
            run(" see you at the sta", &[]),
        ]
    );
}

#[test]
fn stray_close_is_ignored() {
    // x</b>y
    assert_eq!(
        parse(vec![Text("x"), Close(Tag::Bold), Text("y")]),
        vec![run("xy", &[])]
    );
}

#[test]
fn crossed_tags_keep_the_pair_that_closes_on_top() {
    // <b><i>x</b>y</i>: </b> doesn't close the top, </i> does
    assert_eq!(
        parse(vec![
            Open(Tag::Bold),
            Open(Tag::Italic),
            Text("x"),
            Close(Tag::Bold),
            Text("y"),
            Close(Tag::Italic),
        ]),
        vec![run("xy", &[Tag::Italic])]
    );
}

#[test]
fn neighbouring_runs_with_one_style_merge() {
    // <b> hello world </b><b> hello </b>  vs  the same with a space between
    let joined = vec![
        Open(Tag::Bold),
        Text(" hello world "),
        Close(Tag::Bold),
        Open(Tag::Bold),
        Text(" hello "),
        Close(Tag::Bold),
    ];
    assert_eq!(
        parse(joined),
        vec![run(" hello world  hello ", &[Tag::Bold])]
    );

    let spaced = vec![
        Open(Tag::Bold),
        Text(" hello world "),
        Close(Tag::Bold),
        Text(" "),
        Open(Tag::Bold),
        Text(" hello "),
        Close(Tag::Bold),
    ];
    assert_eq!(
        parse(spaced),
        vec![
            run(" hello world ", &[Tag::Bold]),
            run(" ", &[]),
            run(" hello ", &[Tag::Bold]),
        ]
    );
}

#[test]
fn nothing_in_nothing_out() {
    assert_eq!(parse(vec![]), vec![]);
}
