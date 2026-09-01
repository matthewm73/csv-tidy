# csvtidy

A small command-line tool that parses a CSV file, checks that it's
structurally sound (every row has the same number of columns as the
header), and prints it back out either as an aligned table or as JSON.

Most CSV problems I run into aren't about the values, they're about the
shape: a stray comma inside an unquoted field, a row that's missing a
trailing column, a quote that never gets closed. `csvtidy` is meant to
catch that class of problem before it gets further downstream, and to
give both a human and a script a way to look at the result.

## Building

Standard library only, no dependencies to fetch:

```
cargo build --release
```

The binary ends up at `target/release/csvtidy`.

## Usage

```
csvtidy [OPTIONS] [FILE]
```

If `FILE` is omitted, or is `-`, input is read from stdin.

```
OPTIONS:
    --json         emit machine-readable JSON instead of a table
    --no-header    treat every row as data (no header row)
    -h, --help     show help text
```

### Example

Given `people.csv`:

```csv
name,city,age
"Doe, Jane",Springfield,41
Marco Diaz,Toledo,29
```

Pretty-printed table:

```
$ csvtidy people.csv
┌────────────┬─────────────┬─────┐
│ name       │ city        │ age │
├────────────┼─────────────┼─────┤
│ Doe, Jane  │ Springfield │ 41  │
│ Marco Diaz │ Toledo      │ 29  │
└────────────┴─────────────┴─────┘
```

Same file, JSON mode:

```
$ csvtidy --json people.csv
{"valid":true,"header":["name","city","age"],"column_count":3,"row_count":2,"rows":[["Doe, Jane","Springfield","41"],["Marco Diaz","Toledo","29"]]}
```

### A malformed file

```csv
name,city,age
"Doe, Jane",Springfield,41
Marco Diaz,Toledo
```

```
$ csvtidy bad.csv
csvtidy: input failed validation:
  line 3: expected 3 column(s), found 2

$ csvtidy --json bad.csv
{"valid":false,"errors":["line 3: expected 3 column(s), found 2"]}
```

## Quoting rules

Fields follow RFC 4180: wrap a field in double quotes to let it contain
commas or newlines, and double up a quote (`""`) inside a quoted field to
represent a literal `"`. A bare quote in the middle of an unquoted field,
or stray characters after a closing quote, are treated as parse errors
rather than silently accepted, since the goal here is to catch malformed
input, not paper over it.

## Status

Early skeleton: the tokenizer, validator, table printer, and JSON printer
all work end to end, but there's no delimiter option, no streaming for
very large files, and error messages could use more context. See the
roadmap for what's next.
