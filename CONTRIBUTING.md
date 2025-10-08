# Contributing

## Testing

### Snapshot Testing

ClickHouse Analyzer uses [insta](https://insta.rs/docs/cli/) for snapshot testing parse outputs.

When running `cargo test`, the parser will run on each SQL file in `test/inputs`. The output will be compared to "snapshots" of the previous output, which are kept in `test/snapshots`. If the new outputs differ from previous outputs, the tests will fail. New outputs can be accepted using the [`cargo insta review`](https://insta.rs/docs/quickstart/#reviewing-snapshots) command. Updates to snapshots should be committed to the repository and reviewed with related code changes.