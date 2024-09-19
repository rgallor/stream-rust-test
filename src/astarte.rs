// This file is part of Astarte.
//
// Copyright 2024 SECO Mind Srl
//
// SPDX-License-Identifier: Apache-2.0

//! Astarte builder and configuration structures.

use crate::cli::Config;
use crate::math::BaseValue;
use astarte_device_sdk::store::SqliteStore;
use astarte_device_sdk::transport::mqtt::Credential;
use astarte_device_sdk::{Client, DeviceClient};
use color_eyre::eyre;
use color_eyre::eyre::OptionExt;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use std::{env, io};
use tracing::error;
use tracing::log::debug;

/// [`AstarteConfig`] Builder
#[derive(Debug, Default, Deserialize)]
pub struct AstarteConfigBuilder {
    /// Astarte realm
    realm: Option<String>,
    /// Device ID
    device_id: Option<String>,
    /// Device credential
    #[serde(flatten)]
    credential: Option<Credential>,
    /// Astarte pairing url
    pairing_url: Option<String>,
    /// Astarte store directory
    store_directory: Option<PathBuf>,
    /// Flag to ignore Astarte SSL errors
    astarte_ignore_ssl: Option<bool>,
}

impl AstarteConfigBuilder {
    /// Init astarte config from env var if set
    pub fn from_env() -> Self {
        let realm = env::var("ASTARTE_REALM").ok();
        let device_id = env::var("ASTARTE_DEVICE_ID").ok();
        let pairing_url = env::var("ASTARTE_PAIRING_URL").ok();
        let store_directory = env::var("ASTARTE_STORE_DIRECTORY").ok().map(PathBuf::from);
        let astarte_ignore_ssl = env::var("ASTARTE_IGNORE_SSL_ERRORS")
            .map(|s| s.to_lowercase() == "true")
            .ok();
        let credential = env::var("ASTARTE_CREDENTIALS_SECRET")
            .ok()
            .map(Credential::secret)
            .or_else(|| {
                env::var("ASTARTE_PAIRING_TOKEN")
                    .ok()
                    .map(Credential::paring_token)
            });

        Self {
            realm,
            device_id,
            credential,
            pairing_url,
            store_directory,
            astarte_ignore_ssl,
        }
    }

    /// Update the missing config values taking them from a config.toml file
    ///
    /// docker  -> path = "/etc/stream-rust-test/config.toml"
    /// default -> path = "astarte-device-conf/config.toml"
    pub async fn update_with_toml(&mut self, path: impl AsRef<Path>) {
        match tokio::fs::read_to_string(&path).await {
            Ok(file) => {
                // retrieve the astarte config information from the config.toml file
                match toml::from_str::<AstarteConfigBuilder>(&file) {
                    Ok(toml_cfg) => {
                        // update the configs
                        self.merge(toml_cfg);
                    }
                    Err(err) => {
                        error!("error deserializing astarte cfg from toml: {err}");
                    }
                }
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                error!("file {} not found", path.as_ref().display());
            }
            Err(err) => {
                error!("error trying to read {}: {err}", path.as_ref().display());
            }
        }
    }

    /// Merge two configs
    ///
    /// Prioritize the already existing fields
    fn merge(&mut self, other: AstarteConfigBuilder) {
        self.realm = self.realm.take().or(other.realm);
        self.device_id = self.device_id.take().or(other.device_id);
        self.credential = self.credential.take().or(other.credential);
        self.pairing_url = self.pairing_url.take().or(other.pairing_url);
        self.store_directory = self.store_directory.take().or(other.store_directory);
        self.astarte_ignore_ssl = self.astarte_ignore_ssl.take().or(other.astarte_ignore_ssl);
    }

    /// Build a complete Astarte configuration or return an error
    pub fn build(self) -> eyre::Result<AstarteConfig> {
        Ok(AstarteConfig {
            realm: self.realm.ok_or_eyre("missing realm")?,
            device_id: self.device_id.ok_or_eyre("missing device id")?,
            credential: self
                .credential
                .ok_or_eyre("missing either a credential secret or a pairing token")?,
            pairing_url: self.pairing_url.ok_or_eyre("missing pairing url")?,
            store_directory: self.store_directory.ok_or_eyre("missing store directory")?,
            // if missing, set the ignore ssl error flat to false
            astarte_ignore_ssl: self.astarte_ignore_ssl.unwrap_or_default(),
        })
    }
}

/// Astarte device configuration
#[derive(Debug)]
pub struct AstarteConfig {
    /// Astarte realm
    pub realm: String,
    /// Device ID
    pub device_id: String,
    /// Device credential
    pub credential: Credential,
    /// Astarte pairing url
    pub pairing_url: String,
    /// Astarte store directory
    pub store_directory: PathBuf,
    /// Flag to ignore Astarte SSL errors
    pub astarte_ignore_ssl: bool,
}

/// Send data to Astarte
pub async fn send_data(
    client: DeviceClient<SqliteStore>,
    now: SystemTime,
    cfg: Config,
) -> eyre::Result<()> {
    let mut base_value = BaseValue::try_from_system_time(now, cfg.scale)?;

    debug!(
        "sending data to Astarte with {} math function",
        cfg.math_function
    );

    loop {
        // Send data to Astarte
        let value = cfg.math_function.compute(base_value.value());

        client
            .send(&cfg.interface_datastream_do, "/test/value", value)
            .await?;

        debug!("data sent on endpoint /test/value, content: {value}");

        // update the data to send at the next iteration
        base_value.update();

        // Sleep interval secs
        tokio::time::sleep(std::time::Duration::from_millis(cfg.interval_btw_samples)).await;
    }
}
