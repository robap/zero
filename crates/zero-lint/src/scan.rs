//! Minimal hand-rolled SCSS scanner.
//!
//! Walks character-by-character tracking `{ } ;` to produce a stream of
//! [`Decl`] entries grouped by selector. Sass nesting is flattened by
//! concatenating selectors (parent + descendant joined with `' '`); the
//! `&` selector is treated literally. Comments are dropped.
//!
//! The scanner is intentionally not a full SCSS parser — it ignores
//! `@use`, `@mixin`, function calls, and SCSS interpolation. Rules only
//! need property/value text plus enough position info to render a
//! caret-pointed diagnostic.

use std::collections::HashMap;

/// One CSS declaration captured by [`scan`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decl {
    pub property: String,
    pub value: String,
    /// 1-based line where the property name starts.
    pub line: u32,
    /// 1-based column where the property name starts.
    pub column: u32,
    /// Flattened selector path leading to this declaration, joined with `' '`.
    pub selector_path: Vec<String>,
}

/// One rule body — the declarations directly inside `selector { … }`,
/// plus the declarations of nested rules keyed by combinator selector.
#[derive(Debug, Clone, Default)]
pub struct RuleBody {
    pub selector: String,
    pub decls: Vec<Decl>,
    /// Map keyed by nested selector text (e.g. `"> :first-child"`).
    pub child_decls: HashMap<String, Vec<Decl>>,
}

/// Scan `source` and return every captured declaration plus the rule
/// bodies needed for body-shaped rules.
pub fn scan(source: &str) -> (Vec<Decl>, Vec<RuleBody>) {
    let stripped = strip_comments(source);
    let mut state = State::new(&stripped);
    state.scan();
    (state.decls, state.bodies)
}

/// Strip `// line` and `/* block */` comments, preserving line numbers by
/// replacing comment bytes with spaces (newlines kept intact).
fn strip_comments(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                out.push(b' ');
                i += 1;
            }
        } else if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            out.push(b' ');
            out.push(b' ');
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                out.push(if bytes[i] == b'\n' { b'\n' } else { b' ' });
                i += 1;
            }
            if i + 1 < bytes.len() {
                out.push(b' ');
                out.push(b' ');
                i += 2;
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| src.to_string())
}

struct State<'a> {
    src: &'a [u8],
    i: usize,
    line: u32,
    col: u32,
    selector_stack: Vec<String>,
    /// Body stack mirrors selector_stack; each entry collects decls and
    /// child decls for that nesting level. On `}` we pop and merge the
    /// closed level into its parent's `child_decls` if there is one.
    body_stack: Vec<RuleBody>,
    decls: Vec<Decl>,
    bodies: Vec<RuleBody>,
}

impl<'a> State<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src: src.as_bytes(),
            i: 0,
            line: 1,
            col: 1,
            selector_stack: Vec::new(),
            body_stack: Vec::new(),
            decls: Vec::new(),
            bodies: Vec::new(),
        }
    }

    fn scan(&mut self) {
        while self.i < self.src.len() {
            self.skip_ws_and_at_rules();
            if self.i >= self.src.len() {
                break;
            }
            let ch = self.src[self.i];
            if ch == b'}' {
                self.close_block();
                self.bump();
                continue;
            }
            // Lookahead: is this a rule header or a declaration? Find the
            // next `{` or `;` outside of parens/strings within the segment.
            match self.find_segment_end() {
                SegmentKind::Block(end) => {
                    let header = self.slice(self.i, end).trim().to_string();
                    self.advance_to(end);
                    self.bump(); // consume '{'
                    self.open_block(header);
                }
                SegmentKind::Decl(end) => {
                    self.capture_decl(self.i, end);
                    self.advance_to(end);
                    if self.i < self.src.len() && self.src[self.i] == b';' {
                        self.bump();
                    }
                }
                SegmentKind::Eof => break,
            }
        }
        // Flush any unterminated top-level bodies (defensive — most inputs end
        // with a matching `}`).
        while let Some(body) = self.body_stack.pop() {
            self.bodies.push(body);
        }
    }

    fn open_block(&mut self, header: String) {
        let selector = if self.selector_stack.is_empty() {
            header
        } else {
            format!("{} {}", self.selector_stack.last().unwrap(), header)
        };
        let body = RuleBody {
            selector: selector.clone(),
            ..RuleBody::default()
        };
        self.selector_stack.push(selector);
        self.body_stack.push(body);
    }

    fn close_block(&mut self) {
        let Some(closed) = self.body_stack.pop() else {
            return;
        };
        let _ = self.selector_stack.pop();
        // The closed body becomes a top-level entry. If it has a parent
        // body, we also record its declarations under the parent's
        // `child_decls` keyed by the closed body's local selector header.
        if let Some(parent) = self.body_stack.last_mut() {
            // Extract the local header (text after the last single space).
            let key = local_header(&parent.selector, &closed.selector);
            parent
                .child_decls
                .entry(key)
                .or_default()
                .extend(closed.decls.iter().cloned());
        }
        self.bodies.push(closed);
    }

    fn capture_decl(&mut self, start: usize, end: usize) {
        let text = self.slice(start, end);
        let Some(colon) = text.find(':') else {
            return;
        };
        let property = text[..colon].trim().to_string();
        if property.is_empty() || property.starts_with('@') || property.starts_with('$') {
            return;
        }
        // Reject obvious selector-looking heads (e.g. pseudo-class on the
        // left). A property is a plain identifier (letters, digits, `-`).
        if !property
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return;
        }
        let value = text[colon + 1..].trim().trim_end_matches(';').to_string();
        if value.is_empty() {
            return;
        }
        // Position of the first non-whitespace byte of the property in 1-based
        // line/col coordinates.
        let (line, column) = self.line_col_of_first_nonws(start);
        let decl = Decl {
            property,
            value,
            line,
            column,
            selector_path: self.selector_stack.clone(),
        };
        if let Some(body) = self.body_stack.last_mut() {
            body.decls.push(decl.clone());
        }
        self.decls.push(decl);
    }

    fn find_segment_end(&self) -> SegmentKind {
        let mut depth_paren = 0i32;
        let mut j = self.i;
        let mut in_str: Option<u8> = None;
        while j < self.src.len() {
            let c = self.src[j];
            if let Some(q) = in_str {
                if c == q && (j == 0 || self.src[j - 1] != b'\\') {
                    in_str = None;
                }
                j += 1;
                continue;
            }
            match c {
                b'"' | b'\'' => {
                    in_str = Some(c);
                }
                b'(' => depth_paren += 1,
                b')' => depth_paren -= 1,
                b'{' if depth_paren == 0 => return SegmentKind::Block(j),
                b';' if depth_paren == 0 => return SegmentKind::Decl(j),
                b'}' if depth_paren == 0 => return SegmentKind::Decl(j),
                _ => {}
            }
            j += 1;
        }
        SegmentKind::Eof
    }

    fn skip_ws_and_at_rules(&mut self) {
        loop {
            while self.i < self.src.len() && self.src[self.i].is_ascii_whitespace() {
                self.bump();
            }
            if self.i + 4 < self.src.len() && &self.src[self.i..self.i + 4] == b"@use" {
                self.skip_until_semicolon_or_block();
                continue;
            }
            if self.i + 8 < self.src.len() && &self.src[self.i..self.i + 8] == b"@forward" {
                self.skip_until_semicolon_or_block();
                continue;
            }
            if self.i + 7 < self.src.len() && &self.src[self.i..self.i + 7] == b"@import" {
                self.skip_until_semicolon_or_block();
                continue;
            }
            break;
        }
    }

    fn skip_until_semicolon_or_block(&mut self) {
        while self.i < self.src.len() {
            let c = self.src[self.i];
            self.bump();
            if c == b';' || c == b'{' || c == b'}' {
                break;
            }
        }
    }

    fn slice(&self, a: usize, b: usize) -> &str {
        std::str::from_utf8(&self.src[a..b]).unwrap_or("")
    }

    fn bump(&mut self) {
        if self.i < self.src.len() {
            if self.src[self.i] == b'\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
            self.i += 1;
        }
    }

    fn advance_to(&mut self, target: usize) {
        while self.i < target {
            self.bump();
        }
    }

    fn line_col_of_first_nonws(&self, start: usize) -> (u32, u32) {
        let mut line = 1u32;
        let mut col = 1u32;
        for (idx, &b) in self.src.iter().enumerate() {
            if idx == start {
                break;
            }
            if b == b'\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        let mut j = start;
        while j < self.src.len() && self.src[j].is_ascii_whitespace() {
            if self.src[j] == b'\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
            j += 1;
        }
        (line, col)
    }
}

enum SegmentKind {
    Block(usize),
    Decl(usize),
    Eof,
}

/// Recover the local header from a child selector by stripping the
/// parent prefix + the joining space.
fn local_header(parent: &str, child: &str) -> String {
    if let Some(rest) = child.strip_prefix(parent) {
        rest.trim_start().to_string()
    } else {
        child.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_nested_decls_with_line_col() {
        let src = "\
.foo {
  color: red;
  .bar {
    padding: 8px;
  }
}
";
        let (decls, _) = scan(src);
        assert_eq!(decls.len(), 2, "decls = {decls:#?}");
        let color = &decls[0];
        assert_eq!(color.property, "color");
        assert_eq!(color.value, "red");
        assert_eq!(color.line, 2);
        assert_eq!(color.column, 3);

        let padding = &decls[1];
        assert_eq!(padding.property, "padding");
        assert_eq!(padding.value, "8px");
        assert_eq!(padding.line, 4);
        assert_eq!(padding.column, 5);
        assert_eq!(
            padding.selector_path,
            vec![".foo".to_string(), ".foo .bar".to_string()]
        );
    }

    #[test]
    fn scans_groups_child_combinator_decls_under_parent() {
        let src = "\
.flank {
  display: flex;
  & > :first-child { flex: 0 0 auto; }
}
";
        let (_, bodies) = scan(src);
        // Find the body for `.flank`.
        let parent = bodies
            .iter()
            .find(|b| b.selector == ".flank")
            .expect("missing .flank body");
        assert_eq!(parent.decls.len(), 1);
        assert_eq!(parent.decls[0].property, "display");
        assert!(
            parent
                .child_decls
                .keys()
                .any(|k| k.contains(":first-child")),
            "expected child entry keyed by selector containing `:first-child`, got {:?}",
            parent.child_decls.keys().collect::<Vec<_>>()
        );
        let child = parent
            .child_decls
            .values()
            .next()
            .expect("missing child decls");
        assert!(child.iter().any(|d| d.property == "flex"));
    }
}
