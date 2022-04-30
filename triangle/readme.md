# Triangle

Port of https://github.com/matusnovak/rpi-opengl-without-x to Rust + rust_gr_context

## Status

**2022-04-30**

All code runs, on both RPi3 and RPi4.

## Prerequisites / Tested with

### RPi 4

Tested on _Buster_ with `dtoverlay=vc4-fkms-v3d` in `/boot/config.txt`.

### RPi 3

Tested on _Buster_ with neither `vc4-fkms-v3d` or `vc4-kms-v3d` in `/boot/config.txt`.

## Build / Run

### RPi 3

```sh
RUSTFLAGS='-L /opt/vc/lib' RUST_BACKTRACE=1 cargo run --features=vc4
```

### RPi 4

```sh
RUST_BACKTRACE=1 cargo run --features=vc6
```
