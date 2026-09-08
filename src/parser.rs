//! CSV tokenizing and structural validation.
//!
//! This is deliberately stricter than a "just make it parse" reader: a
//! bare quote in the middle of an unquoted field, or stray text after a
//! closing quote, is treated as an error rather than silently absorbed.
//! The point of this tool is to catch malformed CSV, not paper over it.

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub line: usize,
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
}

impl std::fmt::Display for CsvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CsvError::Parse(e) => write!(f, "{e}"),
            CsvError::Ragged(e) => write!(f, "{e}"),
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

/// Tokenize raw CSV text into records, honoring RFC 4180 quoting: fields
/// may be wrapped in `quote`, a doubled quote character inside a quoted
/// field is a literal instance of it, and quoted fields may contain the
/// delimiter and newlines.
fn tokenize(input: &str, delimiter: char, quote: char) -> Result<Vec<Record>, ParseError> {
    enum State {
        FieldStart,
        Unquoted,
        Quoted,
        AfterQuote,
    }

    let mut records = Vec::new();
    let mut record_fields: Vec<String> = Vec::new();
    let mut record_start_line = 1usize;
    let mut field = String::new();
    let mut state = State::FieldStart;
    let mut line = 1usize;
    let mut col = 1usize;

    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match state {
            State::FieldStart => match c {
                q if q == quote => state = State::Quoted,
                d if d == delimiter => {
                    record_fields.push(std::mem::take(&mut field));
                }
                '\n' => {
                    record_fields.push(std::mem::take(&mut field));
                    records.push(Record { line: record_start_line, fields: std::mem::take(&mut record_fields) });
                    line += 1;
                    col = 0;
                    record_start_line = line;
                }
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    record_fields.push(std::mem::take(&mut field));
                    records.push(Record { line: record_start_line, fields: std::mem::take(&mut record_fields) });
                    line += 1;
                    col = 0;
                    record_start_line = line;
                }
                other => {
                    field.push(other);
                    state = State::Unquoted;
                }
            },
            State::Unquoted => match c {
                d if d == delimiter => {
                    record_fields.push(std::mem::take(&mut field));
                    state = State::FieldStart;
                }
                '\n' => {
                    record_fields.push(std::mem::take(&mut field));
                    records.push(Record { line: record_start_line, fields: std::mem::take(&mut record_fields) });
                    state = State::FieldStart;
                    line += 1;
                    col = 0;
                    record_start_line = line;
                }
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    record_fields.push(std::mem::take(&mut field));
                    records.push(Record { line: record_start_line, fields: std::mem::take(&mut record_fields) });
                    state = State::FieldStart;
                    line += 1;
                    col = 0;
                    record_start_line = line;
                }
                q if q == quote => {
                    return Err(ParseError {
                        line,
                        column: col,
                        message: "quote character is only valid at the start of a field".to_string(),
                    });
                }
                other => field.push(other),
            },
            State::Quoted => {
                if c == quote {
                    if chars.peek() == Some(&quote) {
                        chars.next();
                        field.push(quote);
                    } else {
                        state = State::AfterQuote;
                    }
                } else {
                    if c == '\n' {
                        line += 1;
                        col = 0;
                    }
                    field.push(c);
                }
            }
            State::AfterQuote => match c {
                d if d == delimiter => {
                    record_fields.push(std::mem::take(&mut field));
                    state = State::FieldStart;
                }
                '\n' => {
                    record_fields.push(std::mem::take(&mut field));
                    records.push(Record { line: record_start_line, fields: std::mem::take(&mut record_fields) });
                    state = State::FieldStart;
                    line += 1;
                    col = 0;
                    record_start_line = line;
                }
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    record_fields.push(std::mem::take(&mut field));
                    records.push(Record { line: record_start_line, fields: std::mem::take(&mut record_fields) });
                    state = State::FieldStart;
                    line += 1;
                    col = 0;
                    record_start_line = line;
                }
                _ => {
                    return Err(ParseError {
                        line,
                        column: col,
                        message: "unexpected character after closing quote".to_string(),
                    });
                }
            },
        }
        col += 1;
    }

    match state {
        State::FieldStart => {
            if !record_fields.is_empty() || !field.is_empty() {
                record_fields.push(field);
                records.push(Record { line: record_start_line, fields: record_fields });
            }
        }
        State::Unquoted | State::AfterQuote => {
            record_fields.push(field);
            records.push(Record { line: record_start_line, fields: record_fields });
        }
        State::Quoted => {
            return Err(ParseError {
                line,
                column: col,
                message: "unterminated quoted field".to_string(),
            });
        }
    }

    Ok(records)
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
    let records = tokenize(input, delimiter, quote).map_err(|e| vec![CsvError::Parse(e)])?;

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
        let result = parse("a,b\n\"unterminated,2\n", true, ',', '"');
        assert!(matches!(result, Err(errors) if matches!(errors[0], CsvError::Parse(_))));
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
