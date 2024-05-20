# `moss` and `boulder`

A rewrite of the [Serpent OS](https://serpentos.com) tooling in Rust, enabling a robust implementation befitting Serpent and [Solus](https://getsol.us)

The Rust re-implementations of `moss` and `boulder` have now exceeded the capabilities of the original PoC code bases.

It is recommended to use an up to date version of Rust via `rustup`.


## Status

Current Milestone target: [oxide-prealpha1](https://github.com/serpent-os/moss/milestone/1)

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
 - [x] boulder ported
 - [ ] Features (previously: Subscriptions)


## Onboarding

```bash
# This will build boulder and moss and install them to /usr/local by default
just get-started

# If you want to override the install prefix, do the following:
PREFIX=/usr just get-started
```


## Documentation

See [docs.serpentos.com](https://docs.serpentos.com/).


## Experiment

**NB:** Remember to use the `-D sosroot/` argument to specify a root directory, otherwise moss will happily
eat your current operating system.


```bash
just get-started

# create the sosroot/ directory
mkdir -pv sosroot/

# Add the volatile repo
moss -D sosroot/ repo add volatile https://dev.serpentos.com/volatile/x86_64/stone.index

# List packages
moss -D sosroot/ list available

# Install something
moss -D sosroot/ install systemd bash libx11-32bit
```

If you want to create systemd-nspawn roots or bootable VMs, please check out the [img-tests](https://github.com/serpent-os/img-tests) repository.


## Contributing changes

Please ensure all tests are running locally without issue:

```bash
$ just test

# Prior to committing a change:
$ just test # includes the just lint target

# Prior to pushing anything, apply clippy fixes:
$ just fix
```

Then create a Pull Request with your changes.

## License

`moss-rs` is available under the terms of the [MPL-2.0](https://spdx.org/licenses/MPL-2.0.html)
