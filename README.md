# SystemScope Contracts

Shared contracts for [SystemScope](https://github.com/SystemScopeLabs/systemscope), a
deterministic discrete-event simulation runtime. The `systemscope` repository consumes this
crate; nothing here depends on a particular simulation backend.

`systemscope-contracts` defines the types every component and tool agrees on:

- time: ticks, clock domains, durations, frequencies
- events: phases, event keys, scheduling
- components and ports, with the closed protocol message set (`mem.v0`, `mem.v1`, `irq.v0`,
  `block.v0`)
- snapshots and the canonical encoding used for digests
- trace records and the trace stream format
- read-only observation (`WorldView`, `Observer`)

These types are part of the determinism contract: a change to their encoding changes
digests and snapshot bytes. The specification lives in `docs/m0-design.md` of the
`systemscope` repository.

## Pinned revisions

`systemscope` does not track this repository's `main`. Its CI checks out a specific commit
of `contracts`, and moving to a newer commit is a deliberate change in `systemscope`. A
contract change lands here first; `systemscope` then bumps the pinned revision.

## Build and test

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo nextest run --workspace --locked
cargo test --workspace --doc --locked
```

The toolchain is pinned in `rust-toolchain.toml`.
