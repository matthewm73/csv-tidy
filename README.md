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
    --json              emit machine-readable JSON instead of a table
    --no-header         treat every row as data (no header row)
    --delimiter <CHAR>  field delimiter (default: ,); use \t for tab
    --quote <CHAR>      quote character (default: ")
    --write <FILE>      re-serialize validated input and write it to FILE
    -h, --help          show help text
```

`--delimiter` and `--quote` each take a single character (`--delimiter ;`
or `--delimiter=;`), and must not match each other. This is enough to
handle tab-separated files (`--delimiter '\t'`) or files that use a
single quote instead of a double quote.

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

### Writing validated CSV back out

`--write` re-serializes the parsed table and writes it to a file, using
whatever delimiter and quote character were given for parsing. Fields are
only quoted if they need to be, so a file with no odd characters in it
round-trips byte-for-byte (aside from normalizing line endings to `\n`
and dropping trailing blank lines).

```
$ csvtidy --write clean.csv messy.csv
csvtidy: wrote 2 row(s) to clean.csv
```

If the input fails validation, nothing is written.

## Quoting rules

Fields follow RFC 4180: wrap a field in double quotes to let it contain
commas or newlines, and double up a quote (`""`) inside a quoted field to
represent a literal `"`. A bare quote in the middle of an unquoted field,
or stray characters after a closing quote, are treated as parse errors
rather than silently accepted, since the goal here is to catch malformed
input, not paper over it.

## Status

Early skeleton: the tokenizer, validator, table printer, JSON printer, and
CSV writer all work end to end, and the delimiter and quote character are
configurable, but there's no streaming for very large files yet and error
messages could use more context. See the roadmap for what's next.
