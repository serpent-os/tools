# Boulder

This directory contains the Serpent OS package building tool `boulder`.

## Building boulder

To build boulder, use the `boulder` target:

    cargo build -p boulder

This will produce a debug build by default, which is available as  `./target/debug/boulder`

## Onboarding

Refer to the moss onboarding instructions [here](https://github.com/serpent-os/moss?tab=readme-ov-file#onboarding).

## Concurrency test recipe

Assuming you've followed the onboarding instructions above, you can attempt to run multiple boulder instances on a system at the same time:

    for i in 1 2 3; do
      boulder build ./boulder-concurrency-test.yaml -b"$i" |& tee "build-$i".log &
    done
