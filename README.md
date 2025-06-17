# ClickHouse Analyzer

An analyzer for ClickHouse SQL.

## Goal

The idea for this project is to have a tool like [rust-analyzer](https://rust-analyzer.github.io) for ClickHouse SQL (LSP, auto-complete, code suggestions, analysis, node extraction, etc.)

## Contributing

There's not much code here, it is only able to parse some elements of a `SELECT` query.
Feel free to modify and restructure the project as needed.

To learn more about the theory for this parser, read [this blog post](https://matklad.github.io/2023/05/21/resilient-ll-parsing-tutorial.html).

The code has some sample WASM functions. If you want to build the WASM you can follow [this guide from MDN](https://developer.mozilla.org/en-US/docs/WebAssembly/Guides/Rust_to_Wasm#rust_environment_setup).

## Example output

See `example_output.txt` for example of what the tree looks like.
