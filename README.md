<!--
Copyright 2024 SECO Mind Srl

SPDX-License-Identifier: Apache-2.0
-->

# Astarte Stream Rust Test

[![ci](https://github.com/astarte-platform/stream-rust-test/actions/workflows/ci.yaml/badge.svg)](https://github.com/astarte-platform/stream-rust-test/actions/workflows/ci.yaml)
[![codecov](https://codecov.io/gh/astarte-platform/stream-rust-test/graph/badge.svg?token=wW2Hsm5edX)](https://codecov.io/gh/astarte-platform/stream-rust-test)

Astarte Rust SDK based data stream test.

Requirements
============

* Astarte Device Rust SDK

Getting started
===============

## Choosing a Device ID

A Base64 url-encoded uuid should be used, you can use [astartectl](https://github.com/astarte-platform/astartectl#installation) to generate one:

```bash
astartectl utils device-id generate-random
```

## Configuring the application

Create a configuration directory `astarte-device-DEVICE_ID_HERE-conf` with a `config.toml` file inside it containing:
- `realm`: the name of the Astarte realm.
- `device_id`: the id of the device you want to connect to Astarte.
- `pairing_url`: the URL of the Astarte Pairing endpoint. It should be something like `https://<api url>/pairing`.
- `credentials_secret` or `pairing_token`: the identifiers used to authenticate the device through Astarte. If both are
  present, the credential secret will be used.
- `store_directory`: the directory specifying where persistent data will be saved.
- `interfaces_directory`: the directory where the astarte interfaces used by the device are saved.

## Build and run

Build the application using following commands:
```sh
cargo build --release
```

Then run the application either by running the `run.sh` script inside the `scripts` folder or with the CLI:
```sh
./target/release/stream-rust-test [OPTIONS]
```

The full list of options can be shown with the command:
```sh
cargo run -- -h
```

Several options are available:

- `--device` allows to set the device ID;
- `--function` allows to choose the data generation function (one between `sin`, `noisesin`, `randomspikessin`, `saw`, `rect`, `sinc`, `random`, `x` and a default one);
- `--interval` allows to set the sending interval;
- `--scale` allows to scale the generated result;
