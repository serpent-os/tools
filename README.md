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
 - [-] Plugin system for layered graph of dependencies
 - [ ] Search support
 - [ ] Transactions
 - [ ] Installation support
 - [ ] Removal support
 - [ ] Upgrade support
 - [ ] Trigger integration (usysconf-rs)
 - [ ] GC / cleanups of latent states
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

## License

`moss-rs` is available under the terms of the [MPL-2.0](https://spdx.org/licenses/MPL-2.0.html)

