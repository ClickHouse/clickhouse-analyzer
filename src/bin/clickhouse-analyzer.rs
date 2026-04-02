use std::{env, fs, io::Read};

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut format_mode = false;
    let mut file_arg = None;

    for arg in &args[1..] {
        match arg.as_str() {
            "--format" | "-f" => format_mode = true,
            "-" => file_arg = Some("-"),
            _ if !arg.starts_with('-') => file_arg = Some(arg.as_str()),
            _ => {
                eprintln!("unknown option: {arg}");
                std::process::exit(1);
            }
        }
    }

    let input = match file_arg {
        Some("-") | None => {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf).unwrap();
            buf
        }
        Some(path) => fs::read_to_string(path).unwrap(),
    };

    let result = clickhouse_analyzer::parse(&input);

    if format_mode {
        let formatted =
            clickhouse_analyzer::format(&result.tree, &clickhouse_analyzer::FormatConfig::default(), &result.source);
        print!("{formatted}");
    } else {
        let mut buf = String::new();
        result.tree.print(&mut buf, 0, &result.source);
        print!("{buf}");
    }

    let diagnostics = clickhouse_analyzer::enrich_diagnostics(&result, &input);
    for d in &diagnostics {
        let (line, col) = byte_offset_to_line_col(&input, d.range.0);
        let severity = match d.severity {
            clickhouse_analyzer::Severity::Error => "error",
            clickhouse_analyzer::Severity::Warning => "warning",
            clickhouse_analyzer::Severity::Hint => "hint",
        };
        eprintln!("{severity} at {line}:{col}: {}", d.message);
        if let Some(ref suggestion) = d.suggestion {
            eprintln!("  suggestion: {}", suggestion.message);
        }
        for r in &d.related {
            let (rl, rc) = byte_offset_to_line_col(&input, r.range.0);
            eprintln!("  related {rl}:{rc}: {}", r.message);
        }
    }
}

fn byte_offset_to_line_col(src: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in src.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}
