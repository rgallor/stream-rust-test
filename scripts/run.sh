#!/bin/bash

# This file is part of Astarte.
#
# Copyright 2024 SECO Mind Srl
#
# SPDX-License-Identifier: Apache-2.0

set -eEuo pipefail

# Set Rust environment variables
export RUST_LOG="debug"

# Set application environment variables
export REALM=""
export DEVICE_ID=""
export CREDENTIALS_SECRET=""
#export PAIRING_TOKEN=""
export PAIRING_URL="http://api.astarte.localhost/pairing"
export STORE_DIR="/tmp/stream-rust-test/store"
export IGNORE_SSL_ERRORS="true"
export MATH_FUNCTION="sin"
export INTERVAL_BTW_SAMPLES=500
export SCALE=3

# Navigate to the project root
cd "$(dirname "$0")/.."

# Run the Rust application
# Assuming your compiled binary is located in the `target/debug/` directory
./target/debug/stream-rust-test
