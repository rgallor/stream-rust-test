// This file is part of Astarte.
//
// Copyright 2024 SECO Mind Srl
//
// SPDX-License-Identifier: Apache-2.0

//! Astarte builder and configuration structures.

use crate::cli::Config;
use crate::math::BaseValue;
use astarte_device_sdk::builder::{DeviceBuilder, DeviceSdkBuild};
use astarte_device_sdk::store::SqliteStore;
use astarte_device_sdk::transport::grpc::{Grpc, GrpcConfig};
use astarte_device_sdk::transport::mqtt::{Credential, Mqtt, MqttConfig};
use astarte_device_sdk::{Client, DeviceClient, DeviceConnection};
use clap::ValueEnum;
use color_eyre::eyre;
use color_eyre::eyre::{eyre, ContextCompat, OptionExt, WrapErr};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use std::{env, io};
use tracing::{debug, error, info};

/// Stream Rust test node identifier
const STREAM_RUST_TEST_NODE_UUID: uuid::Uuid = uuid::uuid!("d72a6187-7cf1-44cc-87e8-e991936166dc");

const DEVICE_DATASTREAM: &str =
    include_str!("../interfaces/org.astarte-platform.genericsensors.Values.json");

/// Specify which Astarte library use to connect to Astarte
#[derive(
    Debug, Clone, Copy, Default, Eq, PartialEq, Ord, PartialOrd, clap::ValueEnum, Deserialize,
)]
pub enum AstarteConnection {
    /// Connect through MQTT to Astarte
    #[default]
    #[serde(rename = "mqtt")]
    Mqtt,
    /// Connect through gRPC to an Astarte Message Hub instance already connected to Astarte
    #[serde(rename = "grpc")]
    Grpc,
}

/// Astarte connection config toml
///
/// struct used to deserialize the TOML file
#[derive(Debug, Default, Deserialize)]
struct ConfigToml {
    astarte: ConnectionConfigBuilder,
}

/// Astarte connection config Builder
#[derive(Debug, Default, Deserialize)]
pub struct ConnectionConfigBuilder {
    /// Astarte connection
    #[serde(rename = "connection")]
    astarte_connection: Option<AstarteConnection>,
    /// Astarte store directory
    store_directory: Option<PathBuf>,
    /// Astarte Device SDK config options
    #[serde(rename = "mqtt", default)]
    mqtt_config: Option<MqttConfigBuilder>,
    /// Astarte Message Hub config options
    #[serde(rename = "grpc", default)]
    grpc_config: Option<GrpcConfigBuilder>,
}

impl ConnectionConfigBuilder {
    /// Init astarte config from env var if they have been set
    ///
    /// If an error is returned, it means that one or more environment variables have not been set
    pub fn try_from_env(&mut self) -> eyre::Result<()> {
        let con = env::var("ASTARTE_CONNECTION")
            .map(|s| AstarteConnection::from_str(&s, true))?
            .map_err(|err| eyre!(err))?;

        self.store_directory = Some(env::var("ASTARTE_STORE_DIRECTORY").map(PathBuf::from)?);

        match con {
            AstarteConnection::Mqtt => {
                self.astarte_connection = Some(con);

                // update the mqtt config info
                let device_id = env::var("ASTARTE_DEVICE_ID")?;
                let realm = env::var("ASTARTE_REALM")?;
                let pairing_url = env::var("ASTARTE_PAIRING_URL")?;
                let astarte_ignore_ssl = env::var("ASTARTE_IGNORE_SSL_ERRORS")
                    .map(|s| s.parse::<bool>().unwrap_or_default())?;
                let credential = env::var("ASTARTE_CREDENTIALS_SECRET")
                    .map(Credential::secret)
                    .or_else(|_| env::var("ASTARTE_PAIRING_TOKEN").map(Credential::paring_token))?;

                self.mqtt_config = Some(MqttConfigBuilder {
                    device_id,
                    realm,
                    credential,
                    pairing_url,
                    astarte_ignore_ssl,
                });
            }
            AstarteConnection::Grpc => {
                self.astarte_connection = Some(con);

                // update the mqtt config info
                let endpoint = env::var("ASTARTE_MSGHUB_ENDPOINT")?;

                self.grpc_config = Some(GrpcConfigBuilder { endpoint });
            }
        }

        Ok(())
    }

    /// Update the missing config values taking them from a config.toml file
    pub async fn from_toml(&mut self, path: impl AsRef<Path>) {
        match tokio::fs::read_to_string(&path).await {
            Ok(file) => {
                // retrieve the astarte config information from the config.toml file
                match toml::from_str::<ConfigToml>(&file) {
                    Ok(toml_cfg) => {
                        // update the configs
                        *self = toml_cfg.astarte;
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

    /// Build a complete Astarte configuration or return an error
    pub async fn build(self) -> eyre::Result<(DeviceClient<SqliteStore>, SdkConnection)> {
        let astarte_connection = self
            .astarte_connection
            .ok_or_eyre("missing astarte connection")?;

        let store_directory = self.store_directory.ok_or_eyre("missing store directory")?;

        let store = connect_store(&store_directory).await?;

        match astarte_connection {
            AstarteConnection::Mqtt => {
                let astarte_cfg = self.mqtt_config.ok_or_eyre("invalid mqtt config")?;

                debug!("parsed Astarte Device Sdk config: {:#?}", astarte_cfg);

                // define MQTT configuration options
                let mut mqtt_config = MqttConfig::new(
                    astarte_cfg.realm,
                    astarte_cfg.device_id,
                    astarte_cfg.credential,
                    astarte_cfg.pairing_url,
                );

                if astarte_cfg.astarte_ignore_ssl {
                    mqtt_config.ignore_ssl_errors();
                }

                // connect to Astarte
                let (client, connection) = DeviceBuilder::new()
                    .store(store)
                    .interface_str(DEVICE_DATASTREAM)?
                    .connect(mqtt_config)
                    .await?
                    .build()
                    .await;

                Ok((client, SdkConnection::Mqtt(connection)))
            }
            AstarteConnection::Grpc => {
                let astarte_cfg = self.grpc_config.ok_or_eyre("invalid grpc config")?;

                debug!("parsed Astarte Message Hub config: {:#?}", astarte_cfg);

                let grpc_cfg =
                    GrpcConfig::from_url(STREAM_RUST_TEST_NODE_UUID, astarte_cfg.endpoint)
                        .wrap_err("failed to create a gRPC config")?;

                let (client, connection) = DeviceBuilder::new()
                    .store(store)
                    .interface_str(DEVICE_DATASTREAM)?
                    .connect(grpc_cfg)
                    .await?
                    .build()
                    .await;

                Ok((client, SdkConnection::Grpc(connection)))
            }
        }
    }
}

/// SDK [`DeviceConnection`]
pub enum SdkConnection {
    /// Mqtt [DeviceConnection]
    Mqtt(DeviceConnection<SqliteStore, Mqtt<SqliteStore>>),
    /// Grpc [DeviceConnection]
    Grpc(DeviceConnection<SqliteStore, Grpc<SqliteStore>>),
}

/// Config for an MQTT connection to Astarte
#[derive(Debug, Deserialize)]
struct MqttConfigBuilder {
    /// Device ID
    device_id: String,
    /// Astarte realm
    realm: String,
    /// Device credential
    #[serde(flatten)]
    credential: Credential,
    /// Astarte pairing url
    pairing_url: String,
    /// Flag to ignore Astarte SSL errors
    astarte_ignore_ssl: bool,
}

/// Config for a gRPC connection to an Astarte Message Hub instance
#[derive(Debug, Deserialize)]
struct GrpcConfigBuilder {
    /// The Endpoint of the Astarte Message Hub
    endpoint: String,
}

/// Connect to the store.
pub async fn connect_store<P>(store_dir: P) -> eyre::Result<SqliteStore>
where
    P: AsRef<Path>,
{
    let db_path = store_dir.as_ref().join("database.db");
    let db_path_str = db_path.to_str().wrap_err("wrong store db path")?;

    debug!("connecting to store {db_path_str}");
    let store = SqliteStore::connect_db(db_path_str)
        .await
        .wrap_err("failed to connect to store")?;

    info!("connected to store {db_path_str}");
    Ok(store)
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

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_connection_config_builder_build_failures() {
        // empty config builder cannot build successfully
        let cfg_builder = ConnectionConfigBuilder::default().build().await;
        assert!(cfg_builder.is_err());

        // define store dir for the next tests
        let mut tmp_dir = env::temp_dir();
        tmp_dir.push("stream-rust-test-tests");
        std::fs::create_dir_all(&tmp_dir).expect("failed to create store dir");

        // cannot build successfully only with astarte connection and store path information
        let cfg_builder = ConnectionConfigBuilder {
            astarte_connection: Some(AstarteConnection::Mqtt),
            store_directory: Some(tmp_dir.clone()),
            mqtt_config: Default::default(),
            grpc_config: Default::default(),
        };
        let res = cfg_builder.build().await;
        assert!(res.is_err());

        // TODO: add tests where ENV are not valid (or some are missing)           -> build fails
        // TODO: add tests where config file is not found or the file is not valid -> build fails
    }
}
