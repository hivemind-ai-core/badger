//! Canonical layout: re-indents source by bracket depth (four spaces per
//! level), normalizes spacing within lines (see [`crate::spacing`]), strips
//! trailing whitespace, removes blank lines directly inside braces,
//! collapses runs of blank lines, and ends the file with one newline.
//! Tokens, comments, and string contents are never changed (apart from a
//! trailing comma before a closer on the same line), so formatting
//! preserves meaning.

pub fn format(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    // indentation of the line that opened each open bracket: lines inside
    // brackets indent one level past it, however many brackets that line
    // opened, and a line starting with a closer aligns with it
    let mut stack: Vec<usize> = vec![];
    // state carried across lines
    let mut in_block_comment = 0usize;
    let mut in_raw_string = false;
    let mut blank_run = 0usize;
    let mut prev_opened = false;
    // a line ending in an operator (or `=`, `=>`) continues on the next,
    // which is indented one level past the statement's first line
    let mut continued: Option<usize> = None;
    // lines without their `\n`; a `\r` before it is kept here, and is
    // dropped from code lines (the file's line ending is restored below)
    let lines: Vec<&str> = src.split_inclusive('\n').map(|l| l.strip_suffix('\n').unwrap_or(l)).collect();
    for (li, raw) in lines.iter().enumerate() {
        let line = raw.trim_end();
        if in_raw_string || in_block_comment > 0 {
            // inside multi-line literals and comments: keep the line verbatim
            // (a raw string's trailing spaces and `\r` are part of its value)
            let line = if in_raw_string { raw } else { line };
            out.push_str(line);
            out.push('\n');
            let level = stack.last().map(|l| l + 1).unwrap_or(0);
            scan(line, &mut stack, level, &mut in_block_comment, &mut in_raw_string);
            prev_opened = false;
            continue;
        }
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            blank_run += 1;
            continued = None;
            continue;
        }
        // indentation follows the line as it will be written (spacing can
        // drop a stray leading separator, exposing a closer)
        let spaced = crate::spacing::space_line(trimmed);
        let trimmed = spaced.as_str();
        let closes_first = leading_closers(trimmed);
        let mut indent = if closes_first > 0 && stack.len() >= closes_first {
            stack[stack.len() - closes_first]
        } else {
            stack.last().map(|l| l + 1).unwrap_or(0)
        };
        // a pipeline continuation is indented under its expression
        if trimmed.starts_with("|>") {
            indent += 1;
        }
        if let Some(base) = continued
            && closes_first == 0
        {
            indent = indent.max(base + 1);
        }
        if blank_run > 0 && !prev_opened && !trimmed.starts_with(['}', ')', ']']) && li > 0 && !out.is_empty()
        {
            out.push('\n');
        }
        blank_run = 0;
        for _ in 0..indent {
            out.push_str("    ");
        }
        out.push_str(trimmed);
        let before = stack.len();
        scan(trimmed, &mut stack, indent, &mut in_block_comment, &mut in_raw_string);
        if in_raw_string {
            // a raw string opened on this line: what follows the opening
            // quotes, to the line's end, is part of its value
            out.push_str(&raw[line.len()..]);
        }
        out.push('\n');
        prev_opened = stack.len() > before || trimmed.ends_with(['{', '(', '[']);
        continued = if !in_raw_string && in_block_comment == 0 && ends_continued(trimmed) {
            Some(continued.unwrap_or(indent))
        } else {
            None
        };
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    if out.is_empty() {
        return out;
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    // a file written with `\r\n` keeps it (lines ending in a `\r` of
    // their own, inside raw strings, already have it)
    if src.find('\n').is_some_and(|i| i > 0 && src.as_bytes()[i - 1] == b'\r') {
        let mut crlf = String::with_capacity(out.len() + out.len() / 32);
        let mut prev = '\0';
        for c in out.chars() {
            if c == '\n' && prev != '\r' {
                crlf.push('\r');
            }
            crlf.push(c);
            prev = c;
        }
        return crlf;
    }
    out
}

/// Does a line end in a binary operator, `=`, or `=>` (so the statement
/// continues on the next line)? A line with a comment is not judged.
fn ends_continued(line: &str) -> bool {
    if line.contains("//") || line.contains("/*") || line.contains("*/") || line.contains('"') {
        return false;
    }
    let l = line.trim_end();
    const OPS: [&str; 16] =
        ["=>", "==", "!=", "<=", ">=", "&&", "||", "<<", "=", "+", "-", "*", "/", "%", "|", "^"];
    OPS.iter().any(|op| l.ends_with(op)) && !l.ends_with("->") && !l.ends_with("|>")
}

/// Count closing brackets at the start of a line (they dedent it).
fn leading_closers(s: &str) -> usize {
    s.chars().take_while(|c| matches!(c, '}' | ')' | ']')).count()
}

/// Track the brackets of one line (opened at indentation `level`),
/// skipping strings, chars, and comments.
fn scan(line: &str, stack: &mut Vec<usize>, level: usize, block: &mut usize, raw: &mut bool) {
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if *raw {
            if b[i..].starts_with(b"\"\"\"") {
                *raw = false;
                i += 3;
            } else {
                i += 1;
            }
            continue;
        }
        if *block > 0 {
            if b[i..].starts_with(b"/*") {
                *block += 1;
                i += 2;
            } else if b[i..].starts_with(b"*/") {
                *block -= 1;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => return,
            b'/' if b.get(i + 1) == Some(&b'*') => {
                *block += 1;
                i += 2;
            }
            // a raw string, also interpolated (`$"""`): verbatim to its end
            b'"' | b'$' if b[i..].starts_with(b"\"\"\"") || b[i..].starts_with(b"$\"\"\"") => {
                *raw = true;
                i += if b[i] == b'$' { 4 } else { 3 };
            }
            b'"' | b'$' if b[i] == b'"' || b.get(i + 1) == Some(&b'"') => {
                // an ordinary or interpolated string on one line
                match crate::spacing::string_end(line, i) {
                    Some(e) => i = e,
                    None => return,
                }
            }
            b'\'' => {
                i += 1;
                while i < b.len() && b[i] != b'\'' {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
            }
            b'{' | b'(' | b'[' => {
                stack.push(level);
                i += 1;
            }
            b'}' | b')' | b']' => {
                stack.pop();
                i += 1;
            }
            _ => i += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::format;

    #[test]
    fn reindents_and_is_idempotent() {
        let src = "fn f() {\n  let x = [\n1,\n   2,\n]  \n\n\n\n  if x {\n// comment {\n  \"}{\"\n  }\n}\n";
        let once = format(src);
        assert_eq!(
            once,
            "fn f() {\n    let x = [\n        1,\n        2,\n    ]\n\n    if x {\n        // comment {\n        \"}{\"\n    }\n}\n"
        );
        assert_eq!(format(&once), once);
    }

    #[test]
    fn continuation_lines_are_indented() {
        let src = "fn g(n: Int) -> Int =>\nn * 2\nfn h() -> Int {\n    let t = 1 +\n    2 +\n    3\n    t\n}\n/* a */\nconst k = 1\n";
        let want = "fn g(n: Int) -> Int =>\n    n * 2\nfn h() -> Int {\n    let t = 1 +\n        2 +\n        3\n    t\n}\n/* a */\nconst k = 1\n";
        assert_eq!(format(src), want);
        assert_eq!(format(want), want);
    }

    #[test]
    fn raw_strings_are_verbatim() {
        let src = "let s = \"\"\"\n  keep {\n\"\"\"\nlet t = 1\n";
        assert_eq!(format(src), src);
        // inline raw strings, interpolated too, stay one literal
        let inline = "fn f() -> Unit {\nio.println($\"\"\"\\n{{ok}} {3}\"\"\")\n}\n";
        assert_eq!(format(inline), "fn f() -> Unit {\n    io.println($\"\"\"\\n{{ok}} {3}\"\"\")\n}\n");
        let multi = "let s = $\"\"\"a {x}\n  { b\n\"\"\"\nlet t = 1\n";
        assert_eq!(format(multi), multi);
        // trailing spaces and a tab-only line inside are part of the value
        let spaces = "let s = \"\"\"\na  \n\t\n\"\"\"\n";
        assert_eq!(format(spaces), spaces);
        // so are those on the line that opens it
        let opening = "fn f() {\n    let s = \"\"\"abc   \n\"\"\"\n}\n";
        assert_eq!(format(opening), opening);
    }

    #[test]
    fn line_endings_are_kept() {
        // a CRLF file stays CRLF, and a raw string's own `\r` is kept
        let crlf = "fn f() {\r\nlet s = \"\"\"a\r\nb\"\"\"\r\n}\r\n";
        assert_eq!(format(crlf), "fn f() {\r\n    let s = \"\"\"a\r\nb\"\"\"\r\n}\r\n");
        let mixed = "fn f() {\nlet s = \"\"\"a\r\nb\"\"\"\n}\n";
        assert_eq!(format(mixed), "fn f() {\n    let s = \"\"\"a\r\nb\"\"\"\n}\n");
        assert_eq!(format(&format(crlf)), format(crlf));
    }
}
