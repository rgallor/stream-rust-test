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

You can configure the application either by using a direct MQTT connection to an Astarte instance or by connecting
through gRPC to an Astarte Message Hub instance (already configured and connected to Astarte).

To application can be configured by either using environment variables or a `config.toml` file.

If you want to use environment variables to set up the application, you can set the following:
- `ASTARTE_CONNECTION`: specify which type of connection to Astarte must be used
- `ASTARTE_REALM`: name of the astarte realm
- `ASTARTE_DEVICE_ID`: astarte device id
- `ASTARTE_CREDENTIALS_SECRET`: astarte device credential secret
- `ASTARTE_PAIRING_TOKEN`: token used to register a device and obtain a `credentials_secret`
- `ASTARTE_PAIRING_URL`: address of the astarte broker to connect the device to Astarte
- `ASTARTE_STORE_DIRECTORY`: path to the directory where to store data (e.g., in case of Astarte properties)
- `ASTARTE_IGNORE_SSL_ERRORS`: boolean stating if SSL errors should be ignored (default: false)

Instead, if you want to use a configuration file, you must specify its location by using the `ASTARTE_CONFIG_PATH`
environment variable. The `config.toml` file must contain the following information:
```toml
[astarte]
connection = "mqtt" # or "grpc"
store_directory = "/tmp/stream-rust-test/store/"

# MQTT connection to Astarte
[astarte.mqtt]
realm = "REALM_NAME_HERE"
device_id = "DEVICE_ID_HERE"
pairing_url = "PAIRING_URL_HERE"
credentials_secret = "CREDENTIALS_SECRET_HERE"
#pairing_token = "PAIRING_TOKEN_HERE"
astarte_ignore_ssl = false

# gRPC connection to the Astarte Message Hub
[astarte.grpc]
endpoint = "http://[::1]:50051"
```

A detailed description of the fields is depicted below:
- `connection`: a field indicating which type of connection to Astarte should be used.
- `store_directory`: the directory specifying where persistent data will be saved.
- `realm`: the name of the Astarte realm.
- `device_id`: the id of the device you want to connect to Astarte.
- `pairing_url`: the URL of the Astarte Pairing endpoint. It should be something like `https://<api url>/pairing`.
- `credentials_secret` or `pairing_token`: the identifiers used to authenticate the device through Astarte. If both are
  present, the credential secret will be used.
- `astarte_ignore_ssl`: a flag stating if SSL errors should be ignored when connecting to Astarte.
- `endpoint`: the endpoint where the Astarte Message Hub instance is listening for new connections.

Since the application can be configured with a CLI, when [running the application](#build-and-run) you can specify the
type of connection (`mqtt` or `grpc`) and the path to the `config.toml` file with the `--astarte_connection` (`-c`) and
`--astarte_config_path` options, respectively.

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
./target/release/stream-rust-test -h
```

Or you can use the `--help` option (instead of `-h`) to have a more detailed explanation of the available options.

The following options can be set:
- `--device` allows to set the device ID;
- `--function` allows to choose the data generation function (one between `sin`, `noisesin`, `randomspikessin`, `saw`,
  `rect`, `sinc`, `random`, `x` and a default one);
- `--interval` allows to set the sending interval;
- `--scale` allows to scale the generated result;

You can also set the stream options by using the following environment variables:
- `MATH_FUNCTION`
- `INTERFACE_NAME`
- `INTERVAL_BTW_SAMPLES`
- `SCALE`
