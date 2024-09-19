<!--
Copyright 2024 SECO Mind Srl

SPDX-License-Identifier: Apache-2.0
-->

<!--
Copyright 2024 SECO Mind Srl

SPDX-License-Identifier: Apache-2.0
-->

# Config priority

The application can be configured using a CLI, environment variables and a `config.toml` file. In case the configuration
is taken from more sources (e.g., partially from env vars and from the toml file), and one or more configuration
information is provided multiple times, the following priority is used:
1. CLI
2. Environment variables
3. `config.toml`

# Run

first build the application
```shell
cargo build --release 
```

## Run with CLI & `config.toml`

- (ERROR) Only Astarte connection provided (ERROR - missing store dir)
```shell
RUST_LOG=stream_rust_test=debug ./target/release/stream-rust-test -c "mqtt" 
```

- (SUCCESS) Astarte connection provided + `config.toml` (SUCCESS if the toml file is complete)
```shell
RUST_LOG=stream_rust_test=debug ./target/release/stream-rust-test -c "mqtt" -a "astarte-device-conf" 
```

## Run with ENV VAR

- (ERROR) Only Astarte connection provided (ERROR - missing store dir)
```shell
RUST_LOG=stream_rust_test=debug ASTARTE_CONNECTION="mqtt" ./target/release/stream-rust-test 
```

### MQTT

- (ERROR) Astarte connection provided + store directory (ERROR - missing device id)
```shell
RUST_LOG=stream_rust_test=debug ASTARTE_CONNECTION="mqtt" ASTARTE_STORE_DIRECTORY="/tmp/stream-rust-test/store/" ./target/release/stream-rust-test 
```

- (ERROR) Astarte connection provided + store directory + device id (ERROR - missing realm)
```shell
RUST_LOG=stream_rust_test=debug ASTARTE_CONNECTION="mqtt" ASTARTE_STORE_DIRECTORY="/tmp/stream-rust-test/store/" ASTARTE_DEVICE_ID="2TBn-jNESuuHamE2Zo1anA" ./target/release/stream-rust-test 
```

- (ERROR) Astarte connection provided + store directory + device id + realm (ERROR - missing credentials secret or pairing token)
```shell
RUST_LOG=stream_rust_test=debug ASTARTE_CONNECTION="mqtt" ASTARTE_STORE_DIRECTORY="/tmp/stream-rust-test/store/" ASTARTE_DEVICE_ID="2TBn-jNESuuHamE2Zo1anA" ASTARTE_REALM="test" ./target/release/stream-rust-test 
```

- (ERROR) Astarte connection provided + store directory + device id + realm + credentials secret (ERROR - missing pairing url)
```shell
RUST_LOG=stream_rust_test=debug ASTARTE_CONNECTION="mqtt" ASTARTE_STORE_DIRECTORY="/tmp/stream-rust-test/store/" ASTARTE_DEVICE_ID="2TBn-jNESuuHamE2Zo1anA" ASTARTE_REALM="test" ASTARTE_CREDENTIALS_SECRET="QdjYP9UyeuTUd3YH+3Xjd2BPQP79bxU4ewtJS62m/YM=" ./target/release/stream-rust-test 
```

- (ERROR) Astarte connection provided + store directory + device id + realm + credentials secret + pairing url (ERROR - set ignore_ssl_errors to true)
```shell
RUST_LOG=stream_rust_test=debug ASTARTE_CONNECTION="mqtt" ASTARTE_STORE_DIRECTORY="/tmp/stream-rust-test/store/" ASTARTE_DEVICE_ID="2TBn-jNESuuHamE2Zo1anA" ASTARTE_REALM="test" ASTARTE_CREDENTIALS_SECRET="QdjYP9UyeuTUd3YH+3Xjd2BPQP79bxU4ewtJS62m/YM=" ASTARTE_PAIRING_URL="http://api.astarte.localhost/pairing" ./target/release/stream-rust-test 
```

- (SUCCESS) Astarte connection provided + store directory + device id + realm + credentials secret + pairing url + ignore ssl errors
```shell
RUST_LOG=stream_rust_test=debug ASTARTE_CONNECTION="mqtt" ASTARTE_STORE_DIRECTORY="/tmp/stream-rust-test/store/" ASTARTE_DEVICE_ID="2TBn-jNESuuHamE2Zo1anA" ASTARTE_REALM="test" ASTARTE_CREDENTIALS_SECRET="QdjYP9UyeuTUd3YH+3Xjd2BPQP79bxU4ewtJS62m/YM=" ASTARTE_PAIRING_URL="http://api.astarte.localhost/pairing" ASTARTE_IGNORE_SSL_ERRORS="true" ./target/release/stream-rust-test 
```

### gRPC

- (ERROR) Astarte connection provided + store directory (ERROR - missing endpoint)
```shell
RUST_LOG=stream_rust_test=debug ASTARTE_CONNECTION="grpc" ASTARTE_STORE_DIRECTORY="/tmp/stream-rust-test/store/" ./target/release/stream-rust-test 
```

- (SUCCESS) Astarte connection provided + store directory + endpoint
```shell
RUST_LOG=stream_rust_test=debug ASTARTE_CONNECTION="grpc" ASTARTE_STORE_DIRECTORY="/tmp/stream-rust-test/store/" ASTARTE_MSGHUB_ENDPOINT="http://127.0.0.1:50051" ./target/release/stream-rust-test 
```

## Run with CLI + ENV VAR + `config.toml`

- (ERROR) CLI uses grpc, ENV VAR provide mqtt config info
```shell
RUST_LOG=stream_rust_test=debug ASTARTE_CONNECTION="mqtt" ASTARTE_STORE_DIRECTORY="/tmp/stream-rust-test/store/" ASTARTE_DEVICE_ID="2TBn-jNESuuHamE2Zo1anA" ASTARTE_REALM="test" ASTARTE_CREDENTIALS_SECRET="QdjYP9UyeuTUd3YH+3Xjd2BPQP79bxU4ewtJS62m/YM=" ASTARTE_PAIRING_URL="http://api.astarte.localhost/pairing" ASTARTE_IGNORE_SSL_ERRORS="true" ./target/release/stream-rust-test -c "grpc"  
```

- (SUCCESS) CLI uses grpc, ENV VAR provide mqtt config info, config.toml provide grpc endpoint info
  (even if the `connection` field is set to mqtt)
```shell
RUST_LOG=stream_rust_test=debug ASTARTE_CONNECTION="mqtt" ASTARTE_STORE_DIRECTORY="/tmp/stream-rust-test/store/" ASTARTE_DEVICE_ID="2TBn-jNESuuHamE2Zo1anA" ASTARTE_REALM="test" ASTARTE_CREDENTIALS_SECRET="QdjYP9UyeuTUd3YH+3Xjd2BPQP79bxU4ewtJS62m/YM=" ASTARTE_PAIRING_URL="http://api.astarte.localhost/pairing" ASTARTE_IGNORE_SSL_ERRORS="true" ./target/release/stream-rust-test -c "grpc" -a "astarte-device-conf"  
```

# Docker run commands

Run with `--env-file`:

```shell
docker run --network="host" --env-file astarte-device-conf/.env stream-rust-test:latest
```

Run by specifying path to `config.toml` (it is necessary to mount into the docker container the directory containing the
`config.toml` file):

```shell
docker run --network="host" -e ASTARTE_CONFIG_PATH="astarte-device-conf" -v ./astarte-device-conf:/astarte-device-conf stream-rust-test:latest
```

Run by specifying all the Astarte ENV variables:

```shell
docker run --network="host" -e ASTARTE_REALM="test" -e ASTARTE_DEVICE_ID="2TBn-jNESuuHamE2Zo1anA" \-e ASTARTE_CREDENTIALS_SECRET="3wVjV6A+iAWmTLopPb5A3lRFCVsKXPhNxFDxw2Aunb0=" -e ASTARTE_PAIRING_URL="http://api.astarte.localhost/pairing" -e ASTARTE_STORE_DIRECTORY="/tmp/stream-rust-test/store/" -e ASTARTE_IGNORE_SSL_ERRORS="true" stream-rust-test:latest
```
