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
            clickhouse_analyzer::format(&result.tree, &clickhouse_analyzer::FormatConfig::default());
        print!("{formatted}");
    } else {
        let mut buf = String::new();
        result.tree.print(&mut buf, 0);
        print!("{buf}");
    }

    for e in &result.errors {
        let (line, col) = byte_offset_to_line_col(&input, e.range.0);
        eprintln!("error at {line}:{col}: {}", e.message);
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
