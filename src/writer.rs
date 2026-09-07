//! Re-serializing a validated table back into CSV text.

use crate::parser::ParsedTable;

/// Serialize a parsed table back to CSV, using the given delimiter and
/// quote character. A field is quoted only when it needs to be (it
/// contains the delimiter, the quote character, or a newline); everything
/// else is written bare. This means round-tripping a file that didn't need
/// any quoting in the first place won't introduce any.
pub fn write_csv(table: &ParsedTable, delimiter: char, quote: char) -> String {
    let mut out = String::new();

    if let Some(header) = &table.header {
        write_record(&mut out, header, delimiter, quote);
    }
    for row in &table.rows {
        write_record(&mut out, &row.fields, delimiter, quote);
    }

    out
}

fn write_record(out: &mut String, fields: &[String], delimiter: char, quote: char) {
    for (i, field) in fields.iter().enumerate() {
        if i > 0 {
            out.push(delimiter);
        }
        write_field(out, field, delimiter, quote);
    }
    out.push('\n');
}

fn write_field(out: &mut String, field: &str, delimiter: char, quote: char) {
    let needs_quoting = field.chars().any(|c| c == delimiter || c == quote || c == '\n' || c == '\r');

    if !needs_quoting {
        out.push_str(field);
        return;
    }

    out.push(quote);
    for c in field.chars() {
        if c == quote {
            out.push(quote);
        }
        out.push(c);
    }
    out.push(quote);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Record;

    #[test]
    fn round_trips_plain_fields() {
        let table = ParsedTable {
            header: Some(vec!["a".to_string(), "b".to_string()]),
            rows: vec![Record { line: 2, fields: vec!["1".to_string(), "2".to_string()] }],
        };
        assert_eq!(write_csv(&table, ',', '"'), "a,b\n1,2\n");
    }

    #[test]
    fn quotes_fields_that_need_it() {
        let table = ParsedTable {
            header: Some(vec!["name".to_string(), "note".to_string()]),
            rows: vec![Record {
                line: 2,
                fields: vec!["Doe, Jane".to_string(), "she said \"hi\"".to_string()],
            }],
        };
        assert_eq!(
            write_csv(&table, ',', '"'),
            "name,note\n\"Doe, Jane\",\"she said \"\"hi\"\"\"\n"
        );
    }

    #[test]
    fn leaves_bare_fields_unquoted() {
        let table = ParsedTable {
            header: None,
            rows: vec![Record { line: 1, fields: vec!["plain".to_string(), "text".to_string()] }],
        };
        assert_eq!(write_csv(&table, ',', '"'), "plain,text\n");
    }

    #[test]
    fn respects_custom_delimiter_and_quote() {
        let table = ParsedTable {
            header: Some(vec!["name".to_string(), "note".to_string()]),
            rows: vec![Record { line: 2, fields: vec!["Doe; Jane".to_string(), "plain".to_string()] }],
        };
        assert_eq!(write_csv(&table, ';', '\''), "name;note\n'Doe; Jane';plain\n");
    }
}
