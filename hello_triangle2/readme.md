# Hello Triangle 2

Port of Raspberry Pi's `/opt/vc/src/hello_pi/hello_triangle2/` demo.

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
