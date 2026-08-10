# DCTP v1 protocol foundation

`dctp-protocol` contains the DCTP v1 wire codec and payload models. It performs no serial I/O.
`dctp-sim` is the deterministic TCP test transport used by the next desktop-client plan.

Parameter reads report the current RAM value and, for persistent parameters, the separately
committed flash value. `PARAM_COMMIT_ACK` reports the canonical CRC and storage generation.
The simulator provides deterministic, time-varying telemetry across its default drive channels.

Developer commands:

```text
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p dctp-sim -- --help
cargo run -p dctp-sim -- --listen 127.0.0.1:7100
cargo run -p dctp-sim --bin generate_vectors -- --check
```

`generate_vectors` commits six DCTP v1 golden frames, including `param-value.bin` and
`param-commit-ack.bin`; run it without `--check` only when intentionally regenerating them.
