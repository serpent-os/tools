# Boulder

This directory contains the Serpent OS package building tool `boulder`.

## Building boulder

To build boulder, use the `boulder` target:

    cargo build -p boulder

This will produce a debug build by default, which is available as  `./target/debug/boulder`

The [onboarding/ repository](https://github.com/serpent-os/onboarding/) is in the process of being updated to default to building the Rust based boulder.

## Configuring user namespaces

Boulder supports building as your own user, using a feature called "user namespaces".

If your username is `bob` with `UID = 1000` and `GID = 1000` then you will need to add the following files with the following contents:

    $ echo 'bob:100000:65536' |sudo tee /etc/subuid
    $ echo 'bob:100000:65536' |sudo tee /etc/subgid    

NB: The above assumes you haven't already configured user namespaces.

You can check your username, UID and GID with `grep ${USER} /etc/passwd`, where your username is the first field, the UID is the third field and the GID is the fourth field:

    $ grep ${USER} /etc/passwd
    bob:x:1000:1000:bob:/home/bob:/bin/bash
