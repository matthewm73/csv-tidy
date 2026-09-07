mod parser;
mod printer;
mod writer;

use std::env;
use std::fs;
use std::io::{self, Read};
use std::process::ExitCode;

struct Args {
    path: Option<String>,
    json: bool,
    has_header: bool,
    delimiter: char,
    quote: char,
    write: Option<String>,
}

fn parse_args(raw: &[String]) -> Result<Args, String> {
    let mut path = None;
    let mut json = false;
    let mut has_header = true;
    let mut delimiter = ',';
    let mut quote = '"';
    let mut write = None;

    let mut i = 0;
    while i < raw.len() {
        let arg = raw[i].as_str();
        match arg {
            "--json" => json = true,
            "--no-header" => has_header = false,
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "--delimiter" => {
                i += 1;
                let val = raw.get(i).ok_or("--delimiter requires a value")?;
                delimiter = parse_char_flag(val, "--delimiter")?;
            }
            "--quote" => {
                i += 1;
                let val = raw.get(i).ok_or("--quote requires a value")?;
                quote = parse_char_flag(val, "--quote")?;
            }
            "--write" => {
                i += 1;
                let val = raw.get(i).ok_or("--write requires a file path")?;
                write = Some(val.clone());
            }
            other if other.starts_with("--delimiter=") => {
                delimiter = parse_char_flag(&other["--delimiter=".len()..], "--delimiter")?;
            }
            other if other.starts_with("--quote=") => {
                quote = parse_char_flag(&other["--quote=".len()..], "--quote")?;
            }
            other if other.starts_with("--write=") => {
                write = Some(other["--write=".len()..].to_string());
            }
            other if other.starts_with('-') && other != "-" => {
                return Err(format!("unknown flag: {other}"));
            }
            other => {
                if path.is_some() {
                    return Err(format!("unexpected extra argument: {other}"));
                }
                path = Some(other.to_string());
            }
        }
        i += 1;
    }

    if delimiter == quote {
        return Err("--delimiter and --quote must be different characters".to_string());
    }

    Ok(Args { path, json, has_header, delimiter, quote, write })
}

/// Parse a single-character flag value. `\t` is accepted as a shorthand for
/// a literal tab, since typing an actual tab on a command line is awkward.
fn parse_char_flag(val: &str, flag: &str) -> Result<char, String> {
    let c = match val {
        "\\t" => '\t',
        _ => {
            let mut chars = val.chars();
            let c = chars
                .next()
                .ok_or_else(|| format!("{flag} requires a single character"))?;
            if chars.next().is_some() {
                return Err(format!("{flag} must be a single character"));
            }
            c
        }
    };

    if c == '\n' || c == '\r' {
        return Err(format!("{flag} cannot be a newline character"));
    }

    Ok(c)
}

fn print_help() {
    println!("csvtidy - validate and pretty-print a CSV file\n");
    println!("USAGE:");
    println!("    csvtidy [OPTIONS] [FILE]\n");
    println!("If FILE is omitted or is \"-\", input is read from stdin.\n");
    println!("OPTIONS:");
    println!("    --json              emit machine-readable JSON instead of a table");
    println!("    --no-header         treat every row as data (no header row)");
    println!("    --delimiter <CHAR>  field delimiter (default: ,); use \\t for tab");
    println!("    --quote <CHAR>      quote character (default: \")");
    println!("    --write <FILE>      re-serialize validated input and write it to FILE");
    println!("    -h, --help          show this help text");
}

fn read_input(path: &Option<String>) -> io::Result<String> {
    match path.as_deref() {
        None | Some("-") => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        }
        Some(p) => fs::read_to_string(p),
    }
}

fn main() -> ExitCode {
    let raw_args: Vec<String> = env::args().skip(1).collect();
    let args = match parse_args(&raw_args) {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("csvtidy: {msg}");
            eprintln!("Try 'csvtidy --help' for usage.");
            return ExitCode::from(2);
        }
    };

    let input = match read_input(&args.path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("csvtidy: could not read input: {e}");
            return ExitCode::from(1);
        }
    };

    match parser::parse(&input, args.has_header, args.delimiter, args.quote) {
        Ok(table) => {
            if let Some(dest) = &args.write {
                let serialized = writer::write_csv(&table, args.delimiter, args.quote);
                if let Err(e) = fs::write(dest, serialized) {
                    eprintln!("csvtidy: could not write {dest}: {e}");
                    return ExitCode::from(1);
                }
                println!("csvtidy: wrote {} row(s) to {dest}", table.rows.len());
            } else if args.json {
                println!("{}", printer::print_json(&table));
            } else {
                print!("{}", printer::print_human(&table));
            }
            ExitCode::SUCCESS
        }
        Err(errors) => {
            if args.json {
                let messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
                println!("{}", printer::print_json_errors(&messages));
            } else {
                eprintln!("csvtidy: input failed validation:");
                for e in &errors {
                    eprintln!("  {e}");
                }
            }
            ExitCode::from(1)
        }
    }
}
