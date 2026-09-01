//! Rendering a parsed table for a human, or as JSON for a script.

use crate::parser::ParsedTable;

/// Render a parsed table as an aligned grid with box-drawing borders.
pub fn print_human(table: &ParsedTable) -> String {
    let mut widths: Vec<usize> = Vec::new();

    if let Some(header) = &table.header {
        for cell in header {
            widths.push(cell.chars().count());
        }
    }
    for row in &table.rows {
        for (i, cell) in row.fields.iter().enumerate() {
            let w = cell.chars().count();
            match widths.get_mut(i) {
                Some(existing) if *existing < w => *existing = w,
                Some(_) => {}
                None => widths.push(w),
            }
        }
    }

    let mut out = String::new();
    if widths.is_empty() {
        return out;
    }

    let write_separator = |out: &mut String, left: &str, mid: &str, right: &str| {
        out.push_str(left);
        for (i, w) in widths.iter().enumerate() {
            out.push_str(&"─".repeat(w + 2));
            if i + 1 < widths.len() {
                out.push_str(mid);
            }
        }
        out.push_str(right);
        out.push('\n');
    };

    let write_row = |out: &mut String, cells: &[String]| {
        out.push('│');
        for (i, w) in widths.iter().enumerate() {
            let cell = cells.get(i).map(|s| s.as_str()).unwrap_or("");
            out.push(' ');
            out.push_str(cell);
            out.push_str(&" ".repeat(w - cell.chars().count()));
            out.push(' ');
            out.push('│');
        }
        out.push('\n');
    };

    write_separator(&mut out, "┌", "┬", "┐");
    if let Some(header) = &table.header {
        write_row(&mut out, header);
        write_separator(&mut out, "├", "┼", "┤");
    }
    for row in &table.rows {
        write_row(&mut out, &row.fields);
    }
    write_separator(&mut out, "└", "┴", "┘");

    out
}

/// Render a parsed table as JSON. Hand-rolled rather than pulling in a
/// dependency: the shape is small and stable, and this crate has no
/// third-party dependencies by design.
pub fn print_json(table: &ParsedTable) -> String {
    let column_count = table
        .header
        .as_ref()
        .map(|h| h.len())
        .unwrap_or_else(|| table.rows.first().map(|r| r.fields.len()).unwrap_or(0));

    let mut out = String::new();
    out.push('{');

    out.push_str("\"valid\":true,");

    out.push_str("\"header\":");
    match &table.header {
        Some(header) => push_string_array(&mut out, header),
        None => out.push_str("null"),
    }
    out.push(',');

    out.push_str("\"column_count\":");
    out.push_str(&column_count.to_string());
    out.push(',');

    out.push_str("\"row_count\":");
    out.push_str(&table.rows.len().to_string());
    out.push(',');

    out.push_str("\"rows\":[");
    for (i, row) in table.rows.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_string_array(&mut out, &row.fields);
    }
    out.push(']');

    out.push('}');
    out
}

/// Render a list of error messages as the JSON error shape.
pub fn print_json_errors(messages: &[String]) -> String {
    let mut out = String::new();
    out.push_str("{\"valid\":false,\"errors\":[");
    for (i, message) in messages.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_json_string(&mut out, message);
    }
    out.push_str("]}");
    out
}

fn push_string_array(out: &mut String, items: &[String]) {
    out.push('[');
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_json_string(out, item);
    }
    out.push(']');
}

fn push_json_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}
