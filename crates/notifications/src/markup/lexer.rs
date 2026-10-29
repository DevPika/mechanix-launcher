#[derive(Debug, PartialEq)]
pub enum Token<'lexer> {
    Open(Tag),
    Text(&'lexer str),
    Close(Tag),
}

#[derive(Debug, Clone, Default)]
pub struct Lexer<'lexer> {
    rest: &'lexer str,
}

#[derive(Debug, PartialEq)]
pub enum Tag {
    Bold,
    Italic,
    Underline,
}

impl<'lexer> Lexer<'lexer> {
    pub fn new(input: &'lexer str) -> Self {
        Self { rest: input }
    }
}

fn tag_tokenize<'lexer>(rest: &'lexer str) -> (Option<Token<'lexer>>, usize) {
    let Some(used) = tag_span(rest) else {
        return (None, 0);
    };
    let inner = &rest[1..used - 1];
    let Some((closing_tag, name, _attrs)) = parse_tag(inner) else {
        return (None, 0);
    };
    match tag_from_name(name) {
        Some(tag) if closing_tag == true => (Some(Token::Close(tag)), used),
        Some(tag) => (Some(Token::Open(tag)), used),
        None => (None, used),
    }
}

fn tag_span(rest: &str) -> Option<usize> {
    let mut iter = rest.chars();
    if iter.next() != Some('<') {
        return None;
    }
    while let Some(ch) = iter.next() {
        if ch == '<' {
            // if contains < as 2nd character, not a tag
            return None;
        }
        if ch == '>' {
            return Some(rest.len() - iter.as_str().len());
        }
    }
    None
}

fn parse_tag(inner: &str) -> Option<(bool, &str, &str)> {
    let closing_tag = inner.find('/') == Some(0);
    let tag_word = if closing_tag == true {
        &inner[1..]
    } else {
        inner
    };
    let name = tag_word
        .split(|ch: char| ch.is_whitespace() || ch == '/')
        .next()
        .unwrap_or("");
    if !name.starts_with(|ch: char| ch.is_ascii_alphabetic()) {
        return None;
    }
    // TODO: consume the attrs
    let attrs = &tag_word[name.len()..];
    Some((closing_tag, name, attrs))
}

fn tag_from_name(name: &str) -> Option<Tag> {
    if name.eq_ignore_ascii_case("b") {
        Some(Tag::Bold)
    } else if name.eq_ignore_ascii_case("i") {
        Some(Tag::Italic)
    } else if name.eq_ignore_ascii_case("u") {
        Some(Tag::Underline)
    } else {
        None
    }
}

#[test]
fn tag_tokenize_cases() {
    assert_eq!(tag_tokenize("<b>"), (Some(Token::Open(Tag::Bold)), 3));
    assert_eq!(tag_tokenize("</b>"), (Some(Token::Close(Tag::Bold)), 4));
    assert_eq!(tag_tokenize("<B >"), (Some(Token::Open(Tag::Bold)), 4));
    assert_eq!(tag_tokenize("</i> x"), (Some(Token::Close(Tag::Italic)), 4));
    assert_eq!(tag_tokenize("<span>"), (None, 6));
    assert_eq!(tag_tokenize("<br/>"), (None, 5));
    assert_eq!(tag_tokenize("<<b>"), (None, 0));
    assert_eq!(tag_tokenize("<>"), (None, 0));
    assert_eq!(tag_tokenize("</>"), (None, 0));
    assert_eq!(tag_tokenize("< b>"), (None, 0));
    assert_eq!(tag_tokenize("x<b>"), (None, 0));
    assert_eq!(tag_tokenize("<b"), (None, 0));
}

fn text_tokenize<'lexer>(rest: &'lexer str) -> (Option<Token<'lexer>>, usize) {
    let mut iter = rest.chars();
    iter.next();
    while let Some(ch) = iter.next() {
        if ch == '<' {
            let at = rest.len() - iter.as_str().len() - 1;
            if tag_tokenize(&rest[at..]).1 > 0 {
                return (Some(Token::Text(&rest[..at])), at);
            }
        }
    }
    (Some(Token::Text(rest)), rest.len())
}

impl<'lexer> Iterator for Lexer<'lexer> {
    type Item = Token<'lexer>;

    fn next(&mut self) -> Option<Self::Item> {
        while !self.rest.is_empty() {
            let (tag_token, used) = tag_tokenize(self.rest);
            if let Some(token) = tag_token {
                self.rest = &self.rest[used..];
                return Some(token);
            } else if used > 0 {
                self.rest = &self.rest[used..];
            } else {
                let (text_token, used) = text_tokenize(self.rest);
                self.rest = &self.rest[used..];
                return text_token;
            }
        }
        return None;
    }
}

#[cfg(test)]
fn lex(input: &str) -> Vec<Token<'_>> {
    Lexer::new(input).collect()
}

#[test]
fn lexer_cases() {
    assert_eq!(lex(""), vec![]);
    assert_eq!(lex("hello"), vec![Token::Text("hello")]);
    assert_eq!(lex("1 < 2 is true"), vec![Token::Text("1 < 2 is true")]);
    assert_eq!(lex("1 < 2 > 0"), vec![Token::Text("1 < 2 > 0")]);
    assert_eq!(lex("<"), vec![Token::Text("<")]);
    assert_eq!(
        lex("<<<<<<b>"),
        vec![Token::Text("<<<<<"), Token::Open(Tag::Bold)]
    );
    assert_eq!(
        lex("<b>>>>>"),
        vec![Token::Open(Tag::Bold), Token::Text(">>>>")]
    );
    assert_eq!(lex("<span>x</span><br/>"), vec![Token::Text("x")]);
    assert_eq!(
        lex("<b><i>"),
        vec![Token::Open(Tag::Bold), Token::Open(Tag::Italic)]
    );
    assert_eq!(
        lex("é<B>ü 😀</b>"),
        vec![
            Token::Text("é"),
            Token::Open(Tag::Bold),
            Token::Text("ü 😀"),
            Token::Close(Tag::Bold),
        ]
    );
}

#[test]
fn many_less_thans_stay_linear() {
    let input = "<".repeat(100_000);
    assert_eq!(lex(&input), vec![Token::Text(&input)]);
}
