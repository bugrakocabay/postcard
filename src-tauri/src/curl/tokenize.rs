//! Shell-style tokenizer for curl commands (bash/zsh quoting and Windows `cmd` escaping).

use super::CurlError;
use std::iter::Peekable;
use std::str::Chars;

/// Splits a pasted command line into arguments.
///
/// Understands `'..'`, `".."`, `$'..'` (ANSI-C), backslash escapes and
/// `\`-newline continuations. Input that contains `^"` or a line ending in `^`
/// is treated as Windows `cmd` text ("Copy as cURL (cmd)") instead.
pub fn tokenize(input: &str) -> Result<Vec<String>, CurlError> {
    if is_cmd_style(input) { tokenize_cmd(input) } else { tokenize_bash(input) }
}

fn is_cmd_style(input: &str) -> bool {
    input.contains("^\"") || input.lines().any(|l| l.trim_end().ends_with('^'))
}

type Cs<'a> = Peekable<Chars<'a>>;

fn tokenize_bash(input: &str) -> Result<Vec<String>, CurlError> {
    let mut chars = input.chars().peekable();
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut started = false;
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                if started {
                    tokens.push(std::mem::take(&mut cur));
                    started = false;
                }
            }
            '\\' => match chars.next() {
                Some('\n') => {}
                Some('\r') => {
                    chars.next_if_eq(&'\n');
                }
                Some(n) => {
                    cur.push(n);
                    started = true;
                }
                None => {
                    cur.push('\\');
                    started = true;
                }
            },
            '\'' => {
                started = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(ch) => cur.push(ch),
                        None => return Err(CurlError::UnterminatedQuote),
                    }
                }
            }
            '"' => {
                started = true;
                read_double(&mut chars, &mut cur)?;
            }
            '$' if chars.peek() == Some(&'\'') => {
                chars.next();
                started = true;
                read_ansi_c(&mut chars, &mut cur)?;
            }
            '$' if chars.peek() == Some(&'"') => {
                chars.next();
                started = true;
                read_double(&mut chars, &mut cur)?;
            }
            _ => {
                cur.push(c);
                started = true;
            }
        }
    }
    if started {
        tokens.push(cur);
    }
    Ok(tokens)
}

/// Reads the rest of a `"..."` string (the opening quote is already consumed).
fn read_double(chars: &mut Cs, out: &mut String) -> Result<(), CurlError> {
    loop {
        match chars.next() {
            None => return Err(CurlError::UnterminatedQuote),
            Some('"') => return Ok(()),
            Some('\\') => match chars.next() {
                None => return Err(CurlError::UnterminatedQuote),
                Some(n @ ('$' | '`' | '"' | '\\')) => out.push(n),
                Some('\n') => {}
                Some('\r') if chars.peek() == Some(&'\n') => {
                    chars.next();
                }
                Some(n) => {
                    out.push('\\');
                    out.push(n);
                }
            },
            Some(ch) => out.push(ch),
        }
    }
}

/// Consumes up to `max` hex digits and returns their value.
fn take_hex(chars: &mut Cs, max: usize) -> Option<u32> {
    let mut value = None;
    for _ in 0..max {
        match chars.peek().and_then(|c| c.to_digit(16)) {
            Some(d) => {
                value = Some(value.unwrap_or(0) * 16 + d);
                chars.next();
            }
            None => break,
        }
    }
    value
}

fn push_char(bytes: &mut Vec<u8>, c: char) {
    let mut buf = [0u8; 4];
    bytes.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
}

/// Reads the rest of a `$'...'` string (the `$'` is already consumed).
fn read_ansi_c(chars: &mut Cs, out: &mut String) -> Result<(), CurlError> {
    let mut bytes: Vec<u8> = Vec::new();
    loop {
        let Some(c) = chars.next() else { return Err(CurlError::UnterminatedQuote) };
        match c {
            '\'' => break,
            '\\' => {
                let Some(e) = chars.next() else { return Err(CurlError::UnterminatedQuote) };
                match e {
                    'n' => bytes.push(b'\n'),
                    't' => bytes.push(b'\t'),
                    'r' => bytes.push(b'\r'),
                    'a' => bytes.push(7),
                    'b' => bytes.push(8),
                    'e' | 'E' => bytes.push(27),
                    'f' => bytes.push(12),
                    'v' => bytes.push(11),
                    '\\' | '\'' | '"' | '?' => bytes.push(e as u8),
                    'x' => match take_hex(chars, 2) {
                        Some(v) => bytes.push(v as u8),
                        None => bytes.extend_from_slice(b"\\x"),
                    },
                    'u' | 'U' => {
                        let max = if e == 'u' { 4 } else { 8 };
                        let Some(mut cp) = take_hex(chars, max) else {
                            bytes.push(b'\\');
                            bytes.push(e as u8);
                            continue;
                        };
                        if (0xD800..0xDC00).contains(&cp) {
                            // High surrogate: try to pair it with a following \uXXXX.
                            let mut look = chars.clone();
                            if look.next() == Some('\\')
                                && look.next() == Some('u')
                                && let Some(lo) = take_hex(&mut look, 4).filter(|lo| (0xDC00..0xE000).contains(lo))
                            {
                                cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                                *chars = look;
                            }
                        }
                        push_char(&mut bytes, char::from_u32(cp).unwrap_or('\u{FFFD}'));
                    }
                    '0'..='7' => {
                        let mut v = e.to_digit(8).unwrap_or(0);
                        for _ in 0..2 {
                            match chars.peek().and_then(|c| c.to_digit(8)) {
                                Some(d) => {
                                    v = v * 8 + d;
                                    chars.next();
                                }
                                None => break,
                            }
                        }
                        bytes.push(v as u8);
                    }
                    other => {
                        bytes.push(b'\\');
                        push_char(&mut bytes, other);
                    }
                }
            }
            other => push_char(&mut bytes, other),
        }
    }
    out.push_str(&String::from_utf8_lossy(&bytes));
    Ok(())
}

/// Windows `cmd` text: first undo cmd.exe's `^` escaping, then split like the MSVC
/// C runtime does (which is how curl.exe sees its arguments).
fn tokenize_cmd(input: &str) -> Result<Vec<String>, CurlError> {
    split_msvc(&unescape_cmd(input))
}

/// Removes cmd.exe caret escapes. A caret before a newline is a line continuation
/// (inside a string it stands for one literal newline). Carets inside unescaped
/// `"..."` are literal.
fn unescape_cmd(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_quote = false;
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_quote = !in_quote;
                out.push('"');
            }
            '^' if !in_quote => match chars.next() {
                Some(n @ ('\r' | '\n')) => {
                    if n == '\r' {
                        chars.next_if_eq(&'\n');
                    }
                    out.push('\n');
                    // Chrome writes an embedded newline as caret + two line breaks.
                    chars.next_if_eq(&'\r');
                    chars.next_if_eq(&'\n');
                }
                Some(n) => out.push(n),
                None => {}
            },
            _ => out.push(c),
        }
    }
    out
}

/// Splits a command line using the MSVC argv rules: `"` toggles quoting, `\"` is a
/// literal quote, backslashes only count before a quote, and `""` inside quotes is a
/// literal quote.
fn split_msvc(line: &str) -> Result<Vec<String>, CurlError> {
    let chars: Vec<char> = line.chars().collect();
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut started = false;
    let mut in_quote = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            let run = chars[i..].iter().take_while(|&&ch| ch == '\\').count();
            started = true;
            if chars.get(i + run) == Some(&'"') {
                cur.extend(std::iter::repeat_n('\\', run / 2));
                i += run;
                if run % 2 == 1 {
                    cur.push('"');
                    i += 1;
                }
            } else {
                cur.extend(std::iter::repeat_n('\\', run));
                i += run;
            }
        } else if c == '"' {
            started = true;
            if in_quote && chars.get(i + 1) == Some(&'"') {
                cur.push('"');
                i += 2;
            } else {
                in_quote = !in_quote;
                i += 1;
            }
        } else if c.is_whitespace() && !in_quote {
            if started {
                tokens.push(std::mem::take(&mut cur));
                started = false;
            }
            i += 1;
        } else {
            cur.push(c);
            started = true;
            i += 1;
        }
    }
    if in_quote {
        return Err(CurlError::UnterminatedQuote);
    }
    if started {
        tokens.push(cur);
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Vec<String> {
        tokenize(s).unwrap()
    }

    #[test]
    fn plain_words_and_whitespace() {
        assert_eq!(t("curl  -X\tPOST\n  http://x"), ["curl", "-X", "POST", "http://x"]);
        assert!(t("   ").is_empty());
    }

    #[test]
    fn single_quotes_are_literal() {
        assert_eq!(t(r#"a 'b "c" \n $x' d"#), ["a", r#"b "c" \n $x"#, "d"]);
    }

    #[test]
    fn double_quotes_with_escapes() {
        assert_eq!(t(r#""it's" "say \"hi\"" "a\\b" "c\d" "$HOME""#), ["it's", r#"say "hi""#, r"a\b", r"c\d", "$HOME"]);
    }

    #[test]
    fn adjacent_segments_join() {
        // Firefox style: single quote inside single quotes.
        assert_eq!(t(r#"'it'\''s' a"b"'c'"#), ["it's", "abc"]);
        assert_eq!(t(r#"''"#), [""]);
        assert_eq!(t(r#"a "" b"#), ["a", "", "b"]);
    }

    #[test]
    fn backslash_newline_continuation() {
        assert_eq!(
            t("curl 'http://x' \\\n  -H 'a: b' \\\r\n  --compressed"),
            ["curl", "http://x", "-H", "a: b", "--compressed"]
        );
        assert_eq!(t("a\\ b"), ["a b"]);
    }

    #[test]
    fn ansi_c_quoting() {
        assert_eq!(t(r"$'it\'s'"), ["it's"]);
        assert_eq!(t(r#"$'a\nb\tc\\d\'e\"f'"#), ["a\nb\tc\\d'e\"f"]);
        assert_eq!(t(r"$'\x41\u00e9\u0021\u20ac'"), ["A\u{e9}!\u{20ac}"]);
        assert_eq!(t(r"$'\ud83d\ude00'"), ["\u{1F600}"]);
        assert_eq!(t(r"$'\U0001F600 \101 \q'"), ["\u{1F600} A \\q"]);
        assert_eq!(t(r#"--data-raw $'{"a":"it\'s"}'"#), ["--data-raw", r#"{"a":"it's"}"#]);
    }

    #[test]
    fn unterminated_quotes_are_errors() {
        assert_eq!(tokenize("curl 'abc"), Err(CurlError::UnterminatedQuote));
        assert_eq!(tokenize("curl \"abc"), Err(CurlError::UnterminatedQuote));
        assert_eq!(tokenize("curl $'abc"), Err(CurlError::UnterminatedQuote));
        assert_eq!(tokenize("curl ^\"abc"), Err(CurlError::UnterminatedQuote));
    }

    #[test]
    fn cmd_style_chrome() {
        let input = "curl ^\"https://api.example.com/v1/users^?page=1^&limit=10^\" ^\r\n  -H ^\"accept: application/json^\" ^\r\n  --data-raw ^\"^{^\\^\"name^\\^\":^\\^\"Jane^\\^\",^\\^\"pct^\\^\":^\\^\"100^%^\\^\"^}^\" ^\r\n  --compressed";
        assert_eq!(
            t(input),
            [
                "curl",
                "https://api.example.com/v1/users?page=1&limit=10",
                "-H",
                "accept: application/json",
                "--data-raw",
                r#"{"name":"Jane","pct":"100%"}"#,
                "--compressed"
            ]
        );
    }

    #[test]
    fn cmd_style_caret_caret_and_newline_in_string() {
        assert_eq!(t("curl ^\"http://x^\" -d ^\"a^^b^\""), ["curl", "http://x", "-d", "a^b"]);
        assert_eq!(t("curl ^\"http://x^\" -d ^\"l1^\n\nl2^\""), ["curl", "http://x", "-d", "l1\nl2"]);
    }

    #[test]
    fn cmd_style_plain_double_quotes() {
        // Old Chrome / Firefox Windows style with a trailing ^ continuation.
        assert_eq!(
            t("curl \"http://x\" ^\n -H \"a: b\" ^\n -d \"{\"\"k\"\":1}\""),
            ["curl", "http://x", "-H", "a: b", "-d", r#"{"k":1}"#]
        );
        assert_eq!(t("curl \"http://x\" ^\n -d \"{\\\"k\\\":1}\""), ["curl", "http://x", "-d", r#"{"k":1}"#]);
        // Backslashes not before a quote stay.
        assert_eq!(t("curl ^\"C:\\Users\\me\\f.txt^\" ^\n x"), ["curl", r"C:\Users\me\f.txt", "x"]);
    }
}
