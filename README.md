# OS `tools` repository

![image](.github/tools_logo.png)

This repository provides the `moss` and `boulder` tools for managing `.stone` packages, the native package format for Serpent OS.
In a nutshell, a `.stone` is a structured binary package format with well defined payloads and headers, including explicit versioning
information to ensure breaking format changes will succeed and be trivially managed for rolling release
operating systems (i.e Serpent OS).

The `v1` format in use defaults to `zstd` compression (per payload) and relies on a split payload model to enable the
deduplication features:

 - Metadata payload: Package metadata, license, with strongly defined types and keys.
 - Layout payload: Describes the filesystem layout of the package when installed.
 - Index payload: Contains the offsets to access the content payload by hash
 - Content payload: Compressed concatanation of all *unique* files in a package, accessible when decompressed via the Index Payload.

Internally `xxhash` is used for empowering content addressable storage as well as integrity checks within the `.stone` format
on a per-payload basis. This deduplication is carried across the entire OS, which is required to be a `usr-merge` system.
This allows for `/usr` as CAS, with `/.moss` containing the private store and roots. For every transaction in `moss`, a new root
is generated in a staging area. Once fully prepared (such as running transaction triggers in containers), the staging tree is swapped with
the `/usr` tree using `renameat2` with `RENAME_EXCHANGE` to ensure atomicity.

`boulder` is the accompanying tool for *generating* `.stone` files, featuring an approach recipe format (`stone.yaml`), whilst also
housing powerful utilities such as automatic subpackage splitting by patterns, emission of package providers (ie `soname()`), and first-class
support for building packages in rootless containers for a clean build environment.

Note that a repository index, a binary build manifest, and a binary package, are *all* stone-format files and can be
correctly identified by a recent version of the `file` utility.

It is recommended to use an up to date version of Rust via `rustup`.

## Status

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
 - [ ] System model
 - [ ] Subscriptions (named dependency paths and providers to augment the model)


## Onboarding

```bash
# clone the serpent-os moss repo somewhere reasonable
mkdir -pv ~/repos/serpent-os/
cd ~/repos/serpent-os/
git clone https://github.com/serpent-os/tools.git
cd tools/

# Install a few prerequisites (this how you'd do it on Serpent OS)
sudo moss it binutils glibc-devel linux-headers clang tar

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
moss -D sosroot/ repo add volatile https://packages.serpentos.com/volatile/x86_64/stone.index

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
