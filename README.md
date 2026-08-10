# DCTP v1 protocol foundation

`dctp-protocol` contains the DCTP v1 wire codec and payload models. It performs no serial I/O.
`dctp-sim` is the deterministic TCP test transport used by the next desktop-client plan.

Developer commands:

```text
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p dctp-sim -- --help
cargo run -p dctp-sim -- --listen 127.0.0.1:7100
cargo run -p dctp-sim --bin generate_vectors -- --check
```
