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

    let tree = clickhouse_analyzer::parse(&input);
    let mut buf = String::new();
    tree.print(&mut buf, 0);
    print!("{buf}");
}
