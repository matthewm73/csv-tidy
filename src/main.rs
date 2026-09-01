mod parser;
mod printer;

use std::env;
use std::fs;
use std::io::{self, Read};
use std::process::ExitCode;

struct Args {
    path: Option<String>,
    json: bool,
    has_header: bool,
}

fn parse_args(raw: &[String]) -> Result<Args, String> {
    let mut path = None;
    let mut json = false;
    let mut has_header = true;

    for arg in raw {
        match arg.as_str() {
            "--json" => json = true,
            "--no-header" => has_header = false,
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
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
    }

    Ok(Args { path, json, has_header })
}

fn print_help() {
    println!("csvtidy - validate and pretty-print a CSV file\n");
    println!("USAGE:");
    println!("    csvtidy [OPTIONS] [FILE]\n");
    println!("If FILE is omitted or is \"-\", input is read from stdin.\n");
    println!("OPTIONS:");
    println!("    --json         emit machine-readable JSON instead of a table");
    println!("    --no-header    treat every row as data (no header row)");
    println!("    -h, --help     show this help text");
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

    match parser::parse(&input, args.has_header) {
        Ok(table) => {
            if args.json {
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
