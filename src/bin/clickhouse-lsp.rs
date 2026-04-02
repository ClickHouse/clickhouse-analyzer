use clickhouse_analyzer::lsp;

#[tokio::main]
async fn main() {
    lsp::run_stdio().await;
}
