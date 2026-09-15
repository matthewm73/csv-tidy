//! CSV tokenizing and structural validation.
//!
//! This is deliberately stricter than a "just make it parse" reader: a
//! bare quote in the middle of an unquoted field, or stray text after a
//! closing quote, is treated as an error rather than silently absorbed.
//! The point of this tool is to catch malformed CSV, not paper over it.
//! Both `ParseError` and `ValidationError` report a 1-based `column`, so
//! either kind of failure points at the same line/column coordinate.

use std::io::Read;

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub line: usize,
    /// 1-based index of the field being parsed when the error occurred,
    /// mirroring `ValidationError::column` so both error kinds point at
    /// the same "line, column" coordinate a spreadsheet user would use.
    pub column: usize,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}, column {}: {}", self.line, self.column, self.message)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
    pub line: usize,
    /// 1-based index of the first field where this row diverges from the
    /// expected column count: the first missing field if the row is short,
    /// or the first unexpected one if it's long.
    pub column: usize,
    pub expected_columns: usize,
    pub found_columns: usize,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let detail = if self.found_columns < self.expected_columns {
            format!("missing {} field(s)", self.expected_columns - self.found_columns)
        } else {
            format!("{} unexpected field(s)", self.found_columns - self.expected_columns)
        };
        write!(
            f,
            "line {}, column {}: expected {} column(s), found {} ({detail} starting here)",
            self.line, self.column, self.expected_columns, self.found_columns
        )
    }
}

#[derive(Debug, Clone)]
pub enum CsvError {
    Parse(ParseError),
    Ragged(ValidationError),
    Io(String),
}

impl std::fmt::Display for CsvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CsvError::Parse(e) => write!(f, "{e}"),
            CsvError::Ragged(e) => write!(f, "{e}"),
            CsvError::Io(msg) => write!(f, "{msg}"),
        }
    }
}

/// A single CSV record together with the source line it started on.
#[derive(Debug, Clone)]
pub struct Record {
    pub line: usize,
    pub fields: Vec<String>,
}

pub struct ParsedTable {
    pub header: Option<Vec<String>>,
    pub rows: Vec<Record>,
}

enum State {
    FieldStart,
    Unquoted,
    Quoted,
    /// Saw `quote` while inside `Quoted`; ambiguous until the next char
    /// arrives (doubled quote vs. end of field).
    QuoteSeen,
    AfterQuote,
}

/// Tokenizes CSV one char at a time, honoring RFC 4180 quoting: fields may
/// be wrapped in `quote`, a doubled quote character inside a quoted field
/// is a literal instance of it, and quoted fields may contain the
/// delimiter and newlines.
///
/// Deciding what a quote or a `\r` means normally takes one char of
/// lookahead, but a chunked reader can't peek past the end of the chunk
/// it currently has. Instead of buffering the whole input to get that
/// lookahead for free, ambiguous chars are resolved by carrying a pending
/// flag/state into the next `push_char` call.
struct Tokenizer {
    delimiter: char,
    quote: char,
    state: State,
    records: Vec<Record>,
    record_fields: Vec<String>,
    record_start_line: usize,
    field: String,
    line: usize,
    just_saw_cr: bool,
}

impl Tokenizer {
    fn new(delimiter: char, quote: char) -> Self {
        Tokenizer {
            delimiter,
            quote,
            state: State::FieldStart,
            records: Vec::new(),
            record_fields: Vec::new(),
            record_start_line: 1,
            field: String::new(),
            line: 1,
            just_saw_cr: false,
        }
    }

    fn end_record(&mut self) {
        self.record_fields.push(std::mem::take(&mut self.field));
        self.records.push(Record {
            line: self.record_start_line,
            fields: std::mem::take(&mut self.record_fields),
        });
        self.line += 1;
        self.record_start_line = self.line;
    }

    fn push_char(&mut self, c: char) -> Result<(), ParseError> {
        if self.just_saw_cr {
            self.just_saw_cr = false;
            if c == '\n' {
                return Ok(());
            }
        }

        match self.state {
            State::FieldStart => match c {
                q if q == self.quote => self.state = State::Quoted,
                d if d == self.delimiter => {
                    self.record_fields.push(std::mem::take(&mut self.field));
                }
                '\n' => self.end_record(),
                '\r' => {
                    self.end_record();
                    self.just_saw_cr = true;
                }
                other => {
                    self.field.push(other);
                    self.state = State::Unquoted;
                }
            },
            State::Unquoted => match c {
                d if d == self.delimiter => {
                    self.record_fields.push(std::mem::take(&mut self.field));
                    self.state = State::FieldStart;
                }
                '\n' => {
                    self.end_record();
                    self.state = State::FieldStart;
                }
                '\r' => {
                    self.end_record();
                    self.state = State::FieldStart;
                    self.just_saw_cr = true;
                }
                q if q == self.quote => {
                    return Err(ParseError {
                        line: self.line,
                        column: self.record_fields.len() + 1,
                        message: "quote character is only valid at the start of a field".to_string(),
                    });
                }
                other => self.field.push(other),
            },
            State::Quoted => {
                if c == self.quote {
                    self.state = State::QuoteSeen;
                } else {
                    if c == '\n' {
                        self.line += 1;
                    }
                    self.field.push(c);
                }
            }
            State::QuoteSeen => {
                if c == self.quote {
                    self.field.push(self.quote);
                    self.state = State::Quoted;
                } else {
                    self.state = State::AfterQuote;
                    return self.push_char(c);
                }
            }
            State::AfterQuote => match c {
                d if d == self.delimiter => {
                    self.record_fields.push(std::mem::take(&mut self.field));
                    self.state = State::FieldStart;
                }
                '\n' => {
                    self.end_record();
                    self.state = State::FieldStart;
                }
                '\r' => {
                    self.end_record();
                    self.state = State::FieldStart;
                    self.just_saw_cr = true;
                }
                _ => {
                    return Err(ParseError {
                        line: self.line,
                        column: self.record_fields.len() + 1,
                        message: "unexpected character after closing quote".to_string(),
                    });
                }
            },
        }

        Ok(())
    }

    fn finish(mut self) -> Result<Vec<Record>, ParseError> {
        match self.state {
            State::FieldStart => {
                if !self.record_fields.is_empty() || !self.field.is_empty() {
                    self.record_fields.push(self.field);
                    self.records.push(Record { line: self.record_start_line, fields: self.record_fields });
                }
            }
            State::Unquoted | State::AfterQuote | State::QuoteSeen => {
                self.record_fields.push(self.field);
                self.records.push(Record { line: self.record_start_line, fields: self.record_fields });
            }
            State::Quoted => {
                return Err(ParseError {
                    line: self.line,
                    column: self.record_fields.len() + 1,
                    message: "unterminated quoted field".to_string(),
                });
            }
        }

        Ok(self.records)
    }
}

/// Reads and decodes the input in fixed-size chunks instead of loading the
/// whole file into memory up front, so tokenizing a multi-gigabyte CSV
/// doesn't first require a multi-gigabyte buffer to hold it in. `carry`
/// holds at most a few leftover bytes of a UTF-8 sequence split across a
/// chunk boundary.
fn tokenize_reader<R: Read>(mut reader: R, delimiter: char, quote: char) -> Result<Vec<Record>, CsvError> {
    let mut tokenizer = Tokenizer::new(delimiter, quote);
    let mut buf = [0u8; 8192];
    let mut carry: Vec<u8> = Vec::new();

    loop {
        let n = reader.read(&mut buf).map_err(|e| CsvError::Io(e.to_string()))?;
        if n == 0 {
            break;
        }
        carry.extend_from_slice(&buf[..n]);

        let consumed = match std::str::from_utf8(&carry) {
            Ok(s) => s.len(),
            Err(e) => {
                if e.error_len().is_some() {
                    return Err(CsvError::Io("input is not valid utf-8".to_string()));
                }
                e.valid_up_to()
            }
        };

        let chunk = std::str::from_utf8(&carry[..consumed]).unwrap();
        for c in chunk.chars() {
            tokenizer.push_char(c).map_err(CsvError::Parse)?;
        }
        carry.drain(..consumed);
    }

    if !carry.is_empty() {
        return Err(CsvError::Io("input is not valid utf-8".to_string()));
    }

    tokenizer.finish().map_err(CsvError::Parse)
}

/// Parse and structurally validate CSV text. Every record must have the
/// same number of fields as the header (or, if `has_header` is false, as
/// the first record). All ragged rows are collected and reported together
/// rather than stopping at the first one, since that's more useful when
/// cleaning up a real file with several bad rows.
///
/// `delimiter` and `quote` must be distinct characters, and neither may be
/// `\r` or `\n`; the caller is expected to have checked this already since
/// those come from command-line input.
pub fn parse(
    input: &str,
    has_header: bool,
    delimiter: char,
    quote: char,
) -> Result<ParsedTable, Vec<CsvError>> {
    parse_reader(input.as_bytes(), has_header, delimiter, quote)
}

/// Same as [`parse`], but reads from any `Read` implementation (a file, a
/// socket, stdin) instead of requiring the caller to have already buffered
/// the whole input into a string.
pub fn parse_reader<R: Read>(
    reader: R,
    has_header: bool,
    delimiter: char,
    quote: char,
) -> Result<ParsedTable, Vec<CsvError>> {
    let records = tokenize_reader(reader, delimiter, quote).map_err(|e| vec![e])?;
    validate(records, has_header)
}

fn validate(records: Vec<Record>, has_header: bool) -> Result<ParsedTable, Vec<CsvError>> {
    if records.is_empty() {
        return Ok(ParsedTable { header: None, rows: Vec::new() });
    }

    let (header, data) = if has_header {
        let mut iter = records.into_iter();
        let header = iter.next().unwrap();
        (Some(header.fields), iter.collect::<Vec<_>>())
    } else {
        (None, records)
    };

    let expected = header
        .as_ref()
        .map(|h| h.len())
        .unwrap_or_else(|| data.first().map(|r| r.fields.len()).unwrap_or(0));

    let errors: Vec<CsvError> = data
        .iter()
        .filter(|r| r.fields.len() != expected)
        .map(|r| {
            CsvError::Ragged(ValidationError {
                line: r.line,
                column: r.fields.len().min(expected) + 1,
                expected_columns: expected,
                found_columns: r.fields.len(),
            })
        })
        .collect();

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(ParsedTable { header, rows: data })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_rows() {
        let table = parse("a,b\n1,2\n3,4\n", true, ',', '"').unwrap();
        assert_eq!(table.header, Some(vec!["a".to_string(), "b".to_string()]));
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0].fields, vec!["1".to_string(), "2".to_string()]);
    }

    #[test]
    fn handles_quoted_commas_and_escaped_quotes() {
        let table = parse("name,note\n\"Doe, Jane\",\"she said \"\"hi\"\"\"\n", true, ',', '"').unwrap();
        assert_eq!(table.rows[0].fields[0], "Doe, Jane");
        assert_eq!(table.rows[0].fields[1], "she said \"hi\"");
    }

    #[test]
    fn rejects_ragged_rows_missing_fields() {
        let result = parse("a,b,c\n1,2\n", true, ',', '"');
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        match &errors[0] {
            CsvError::Ragged(e) => {
                assert_eq!(e.expected_columns, 3);
                assert_eq!(e.found_columns, 2);
                assert_eq!(e.column, 3);
                assert_eq!(
                    e.to_string(),
                    "line 2, column 3: expected 3 column(s), found 2 (missing 1 field(s) starting here)"
                );
            }
            other => panic!("expected a ragged-row error, got {other:?}"),
        }
    }

    #[test]
    fn rejects_ragged_rows_extra_fields() {
        let result = parse("a,b\n1,2,3\n", true, ',', '"');
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        match &errors[0] {
            CsvError::Ragged(e) => {
                assert_eq!(e.expected_columns, 2);
                assert_eq!(e.found_columns, 3);
                assert_eq!(e.column, 3);
                assert_eq!(
                    e.to_string(),
                    "line 2, column 3: expected 2 column(s), found 3 (1 unexpected field(s) starting here)"
                );
            }
            other => panic!("expected a ragged-row error, got {other:?}"),
        }
    }

    #[test]
    fn rejects_unterminated_quote() {
        let result = parse("a,b\n\"unterminated", true, ',', '"');
        let errors = result.unwrap_err();
        match &errors[0] {
            CsvError::Parse(e) => {
                assert_eq!(e.line, 2);
                assert_eq!(e.column, 1);
                assert_eq!(e.to_string(), "line 2, column 1: unterminated quoted field");
            }
            other => panic!("expected a parse error, got {other:?}"),
        }
    }

    #[test]
    fn reports_field_of_stray_quote_in_unquoted_field() {
        let result = parse("a,b\n1,x\"y\n", true, ',', '"');
        let errors = result.unwrap_err();
        match &errors[0] {
            CsvError::Parse(e) => {
                assert_eq!(e.line, 2);
                assert_eq!(e.column, 2);
                assert_eq!(
                    e.to_string(),
                    "line 2, column 2: quote character is only valid at the start of a field"
                );
            }
            other => panic!("expected a parse error, got {other:?}"),
        }
    }

    #[test]
    fn reports_field_of_stray_text_after_closing_quote() {
        let result = parse("a,b\n\"x\"y,z\n", true, ',', '"');
        let errors = result.unwrap_err();
        match &errors[0] {
            CsvError::Parse(e) => {
                assert_eq!(e.line, 2);
                assert_eq!(e.column, 1);
                assert_eq!(
                    e.to_string(),
                    "line 2, column 1: unexpected character after closing quote"
                );
            }
            other => panic!("expected a parse error, got {other:?}"),
        }
    }

    #[test]
    fn supports_tab_delimiter() {
        let table = parse("a\tb\n1\t2\n", true, '\t', '"').unwrap();
        assert_eq!(table.header, Some(vec!["a".to_string(), "b".to_string()]));
        assert_eq!(table.rows[0].fields, vec!["1".to_string(), "2".to_string()]);
    }

    #[test]
    fn supports_semicolon_delimiter_and_single_quote() {
        let table = parse("name;note\n'Doe; Jane';'she said ''hi'''\n", true, ';', '\'').unwrap();
        assert_eq!(table.rows[0].fields[0], "Doe; Jane");
        assert_eq!(table.rows[0].fields[1], "she said 'hi'");
    }
}
