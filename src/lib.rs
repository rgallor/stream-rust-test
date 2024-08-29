// This file is part of Astarte.
//
// Copyright 2024 SECO Mind Srl
//
// SPDX-License-Identifier: Apache-2.0

#![doc = include_str!("../README.md")]
#![warn(clippy::dbg_macro, missing_docs, rustdoc::missing_crate_level_docs)]

pub mod math;

use crate::math::MathFunction;
use color_eyre::eyre;
use color_eyre::eyre::Context;
use std::env;
use std::path::{Path, PathBuf};
use std::str::FromStr;

const INTERFACE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/interfaces");

/// Default interval value expressed in milliseconds between two samples sent to Astarte
const DEFAULT_INTERVAL: u64 = 1000;

/// Default scale for the generation of the data to send to Astarte
const DEFAULT_SCALE: f64 = 1.0;

/// Astarte device configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// Astarte realm
    pub realm: String,
    /// Device ID
    pub device_id: String,
    /// Device credential secret
    pub credentials_secret: Option<String>,
    /// Device pairing token
    pub pairing_token: Option<String>,
    /// Astarte pairing url
    pub pairing_url: String,
    /// Astarte store directory
    pub store_directory: String,
    /// Flag to ignore Astarte SSL errors
    pub ignore_ssl_errors: bool,
    /// Path to folder containing the Astarte Device interfaces
    pub interfaces_folder: PathBuf,
    /// Math function the device will use to send data to Astarte
    pub math_function: MathFunction,
    /// Interface name to send data to
    pub interface_datastream_do: String,
    /// Milliseconds the device must wait before sending data to Astarte
    pub interval_btw_samples: u64,
    /// Scale for the generation of the data to send
    pub scale: f64,
}

fn read_env(name: &str) -> eyre::Result<String> {
    env::var(name).wrap_err_with(|| format!("couldn't read environment variable {name}"))
}

impl Config {
    /// Initialize the device configuration
    pub fn init() -> eyre::Result<Self> {
        // read values from environment variables
        let realm = read_env("REALM")?;
        let device_id = read_env("DEVICE_ID")?;
        let credentials_secret = read_env("CREDENTIALS_SECRET").ok();
        let pairing_token = read_env("PAIRING_TOKEN").ok();
        let pairing_url = read_env("PAIRING_URL")?;
        let store_directory = read_env("STORE_DIR")?;
        let ignore_ssl_errors = read_env("IGNORE_SSL_ERRORS")
            .map(|s| bool::from_str(&s).unwrap_or(true))
            .unwrap_or(true);

        let interfaces_folder = Path::new(INTERFACE_DIR).to_owned();

        let interface_datastream_do = "org.astarte-platform.genericsensors.Values".to_string();

        let math_function = read_env("MATH_FUNCTION").unwrap_or_default().into();

        let interval_btw_samples = env::var("INTERVAL_BTW_SAMPLES")
            .map(|s| s.parse::<u64>().unwrap_or(DEFAULT_INTERVAL))
            .unwrap_or(DEFAULT_INTERVAL);

        let scale =
            env::var("SCALE").map_or(DEFAULT_SCALE, |s| s.parse::<f64>().unwrap_or(DEFAULT_SCALE));

        Ok(Self {
            realm,
            device_id,
            credentials_secret,
            pairing_token,
            pairing_url,
            store_directory,
            ignore_ssl_errors,
            interfaces_folder,
            math_function,
            interface_datastream_do,
            interval_btw_samples,
            scale,
        })
    }
}
