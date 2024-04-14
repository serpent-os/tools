# moss-rs

A rewrite of the [Serpent OS](https://serpentos.com) tooling in Rust, enabling a robust implementation befitting Serpent and [Solus](https://getsol.us)

We will initially focus on `moss` and `stone` (library), restoring parity while focusing on some key areas 

 - Version agnostic read APIs
 - Decoupled repository indices with support for format upgrades
 - Asynchronous I/O (specifically fetches)
 - Memory consumption + safety
 - Upgrade of installations

When the tooling is at a point exceeding the functionality of our first version, focus will shift to the build infrastructure as well
as the Rust version of `boulder` (`.stone` build tooling).

It is recommended to use an up to date version of Rust via `rustup`.

Progress:

 - [x] Read support for `.stone`
 - [x] Repository manipulation
 - [x] Plugin system for layered graph of dependencies
 - [x] Search support
 - [x] Transactions
 - [x] Installation support
 - [x] Removal support
 - [x] `sync` support (See: https://github.com/serpent-os/moss-rs/pull/73#issuecomment-1802672634)
 - [x] Triggers
 - [x] GC / cleanups of latent states
 - [ ] Features (previously: Subscriptions)

## Test libstone

```bash
$ cargo test
```


## Building moss

```bash
$ cargo build -p moss
$ cargo run -p moss -- $args
```

## Experiment

Remember to use the `-D sosroot` argument to specify a root directory, otherwise moss will happily
eat your operating system.

    cargo build --release

    # create the sosroot/ directory
    mkdir -pv sosroot/

    # Add the volatile repo
    ./target/release/moss -D sosroot/ repo add volatile https://dev.serpentos.com/volatile/x86_64/stone.index

    # List packages
    ./target/release/moss -D sosroot/ list available

    # Install something
    ./target/release/moss -D sosroot/ install systemd bash libx11-32bit

## Contributing changes

Please ensure all tests are running locally without issue:

```bash
$ cargo test

# Prior to commiting a change:
$ cargo fmt

# Prior to pushing anything, check:
$ cargo clippy
```

## License

`moss-rs` is available under the terms of the [MPL-2.0](https://spdx.org/licenses/MPL-2.0.html)

