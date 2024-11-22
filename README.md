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
# clone the serpent-os moss repo somewhere reasonable
mkdir -pv ~/repos/serpent-os/
cd ~/repos/serpent-os/
git clone https://github.com/serpent-os/tools.git
cd tools/

# Install a few prerequisites (this how you'd do it on Serpent OS)
sudo moss it binutils glibc-devel linux-headers clang

# remember to add ~/.cargo/bin to your $PATH if this is how you installed rustfmt
cargo install rustfmt

# from inside the moss clone, this will build boulder and moss
# and install them to ${HOME}/.local/bin/ by default
just get-started

# boulder and moss rely on so-called subuid and subgid support.
# IFF you do not already have this set up for your ${USER} in /etc/subuid and /etc/subuid
# you might want to do something similar to this:
sudo touch /etc/sub{uid,gid}
sudo usermod --add-subuids 1000000-1065535 --add-subgids 1000000-1065535 root
sudo usermod --add-subuids 1065536-1131071 --add-subgids 1065536-1131071 ${USER}
```

**NB:** If you want to build .stones with boulder on your _non-serpent_ host system, you will need to specify the
location of the boulder data files (which live in ${HOME}/.local/share/boulder if you used `just get-started` like above):

```bash
alias boulder="${HOME}/.local/bin/boulder --data-dir=${HOME}/.local/share/boulder/ --config-dir=${HOME}/.config/boulder/ --moss-root=${HOME}/.cache/boulder/"
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
