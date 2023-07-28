# BioTracker Version 4

## TL;DR

```
# Build & run test setup
cargo run --release -- --config distribution/test.json
```

To setup SLEAP tracking, look at [distribution/sleap](distribution/sleap/README.md).

## Build Dependencies

To build BioTracker v4, a rust toolchain and OpenCV must be installed.

### Rust

The build requires the Rust toolchain, version >= 1.68. It can be installed by
following the [official guide](https://www.rust-lang.org/tools/install).

### OpenCV 4.x

OpenCV can be installed using your system package manager.

```
# Ubuntu / Debian
sudo apt install libopencv-dev
# Arch
sudo pacman -S opencv
# macOS
brew install opencv
```

## Building

BioTracker v4 is built and executed  with `cargo`. It is recommended to use the
`--release` flag to optimize the build.

```
cargo run --release
```

## Executing

As a modular application, BioTracker v4 requires further configuration
specifying which components should be used for tracking. This is done by
supplying a JSON configuration file at startup, using the `--config` flag. When
using `cargo`, arguments to the BioTracker are passed after a separating
double-dash (` -- `).

The test configuration may be used to quickly run the application. It can can
be executed by calling:

```
cargo run --release -- --config distribution/test.json
```

While this configuration is useful for testing basic features, it does not do
any tracking. Please refer to
[distribution/sleap](distribution/sleap/README.md) for a guide on how to setup
a real tracking pipeline.

## Command Line Interface (CLI)

The CLI may be used to automate some settings at startup. It is documented behind the `--help` argument:

```
biotracker4 --help
Distributed framework for animal tracking

Usage: biotracker4 [OPTIONS] --config <CONFIG>

Options:
  -v, --video <VIDEO>
          Open video file on startup
      --entity-count <ENTITY_COUNT>
          Start experiment with <count> entities
      --realtime <REALTIME>
          Skip frames if tracking is too slow [possible values: true, false]
      --config <CONFIG>
          Path to configuration json
      --port <PORT>
          Port for biotracker core [default: 27342]
      --seek <SEEK>
          Seek to frame
      --cv-worker-threads <CV_WORKER_THREADS>
          Number of OpenCV worker threads [default: 4]
      --track <TRACK>
          Path to robofish track file
      --force-camera-config <FORCE_CAMERA_CONFIG>
          Force loading of camera settings, this makes it possible to apply undistortion to videos
      --port-range-start <PORT_RANGE_START>
          Start of range of ports which are assigned to components [default: 28000]
  -h, --help
          Print help
  -V, --version
          Print version
```

## LICENSE

This work is licensed under GPL 3.0 (or any later version).
Individual files contain the following tag instead of the full license text:

`SPDX-License-Identifier: GPL-3.0-or-later`

This enables machine processing of license information based on the SPDX License Identifiers available here: https://spdx.org/licenses/
