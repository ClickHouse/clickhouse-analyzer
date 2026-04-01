use std::{env, fs, io::Read};

fn main() {
    let args: Vec<String> = env::args().collect();
    let input = match args.get(1).map(|s| s.as_str()) {
        Some("-") | None => {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf).unwrap();
            buf
        }
        Some(path) => fs::read_to_string(path).unwrap(),
    };

    let result = clickhouse_analyzer::parse(&input);
    let mut buf = String::new();
    result.tree.print(&mut buf, 0);
    print!("{buf}");

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
