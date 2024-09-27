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
use color_eyre::eyre::{OptionExt, WrapErr};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use std::{env, io};
use tracing::{debug, error};

/// Stream Rust test node identifier
const STREAM_RUST_TEST_NODE_UUID: uuid::Uuid = uuid::uuid!("d72a6187-7cf1-44cc-87e8-e991936166dc");

const DEVICE_DATASTREAM: &str =
    include_str!("../interfaces/org.astarte-platform.genericsensors.Values.json");

/// Specify which Astarte library use to connect to Astarte
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum, Deserialize)]
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
    mqtt_config: MqttConfigBuilder,
    /// Astarte Message Hub config options
    #[serde(rename = "grpc", default)]
    grpc_config: GrpcConfigBuilder,
}

impl ConnectionConfigBuilder {
    /// Builder constructor
    ///
    /// Specify if the builder should use the Astarte Device SDK or the Astarte Message Hub
    pub fn with_connection(astarte_connection: Option<AstarteConnection>) -> Self {
        Self {
            astarte_connection,
            store_directory: None,
            mqtt_config: MqttConfigBuilder::default(),
            grpc_config: GrpcConfigBuilder::default(),
        }
    }

    /// Init astarte config from env var if they have been set
    pub fn from_env(&mut self) {
        // doesn't change it if it's been set from CLI
        if self.astarte_connection.is_none() {
            self.astarte_connection = env::var("ASTARTE_CONNECTION")
                .ok()
                .map(|s| AstarteConnection::from_str(&s, true))
                .transpose()
                .ok()
                .unwrap_or_default();
        }

        self.store_directory = env::var("ASTARTE_STORE_DIRECTORY").ok().map(PathBuf::from);

        // update the mqtt config info
        let device_id = env::var("ASTARTE_DEVICE_ID").ok();
        let realm = env::var("ASTARTE_REALM").ok();
        let pairing_url = env::var("ASTARTE_PAIRING_URL").ok();
        let astarte_ignore_ssl = env::var("ASTARTE_IGNORE_SSL_ERRORS")
            .map(|s| s.parse().unwrap_or_default())
            .ok();
        let credential = env::var("ASTARTE_CREDENTIALS_SECRET")
            .ok()
            .map(Credential::secret)
            .or_else(|| {
                env::var("ASTARTE_PAIRING_TOKEN")
                    .ok()
                    .map(Credential::paring_token)
            });

        self.mqtt_config = MqttConfigBuilder {
            device_id,
            realm,
            credential,
            pairing_url,
            astarte_ignore_ssl,
        };

        // update the mqtt config info
        let endpoint = env::var("ASTARTE_MSGHUB_ENDPOINT").ok();

        self.grpc_config = GrpcConfigBuilder { endpoint };
    }

    /// Update the missing config values taking them from a config.toml file
    pub async fn update_with_toml(&mut self, path: impl AsRef<Path>) {
        match tokio::fs::read_to_string(&path).await {
            Ok(file) => {
                // retrieve the astarte config information from the config.toml file
                match toml::from_str::<ConfigToml>(&file) {
                    Ok(toml_cfg) => {
                        // update the configs
                        self.merge(toml_cfg.astarte);
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
    fn merge(&mut self, other: ConnectionConfigBuilder) {
        // doesn't change it if it's been set from CLI or from ENV
        if self.astarte_connection.is_none() {
            self.astarte_connection = other.astarte_connection;
        }

        self.store_directory = self.store_directory.take().or(other.store_directory);

        // update the mqtt config info
        let mqtt_config = &mut self.mqtt_config;

        mqtt_config.device_id = mqtt_config.device_id.take().or(other.mqtt_config.device_id);
        mqtt_config.realm = mqtt_config.realm.take().or(other.mqtt_config.realm);
        mqtt_config.credential = mqtt_config
            .credential
            .take()
            .or(other.mqtt_config.credential);
        mqtt_config.pairing_url = mqtt_config
            .pairing_url
            .take()
            .or(other.mqtt_config.pairing_url);
        mqtt_config.astarte_ignore_ssl = mqtt_config
            .astarte_ignore_ssl
            .take()
            .or(other.mqtt_config.astarte_ignore_ssl);

        // update the grpc config info
        let grpc_config = &mut self.grpc_config;

        grpc_config.endpoint = grpc_config.endpoint.take().or(other.grpc_config.endpoint);
    }

    /// Build a complete Astarte configuration or return an error
    pub async fn build(self) -> eyre::Result<(DeviceClient<SqliteStore>, SdkConnection)> {
        let astarte_connection = self
            .astarte_connection
            .ok_or_eyre("missing astarte connection")?;

        let store_directory = self.store_directory.ok_or_eyre("missing store directory")?;

        let builder = DeviceBuilder::new()
            .store_dir(&store_directory)
            .await?
            .interface_str(DEVICE_DATASTREAM)?;

        match astarte_connection {
            AstarteConnection::Mqtt => {
                // define MQTT configuration options
                let mqtt_cfg = self.mqtt_config.build()?;
                debug!("parsed Astarte Device Sdk config: {:#?}", mqtt_cfg);

                // connect to Astarte
                let (client, connection) = builder.connect(mqtt_cfg).await?.build().await;

                Ok((client, SdkConnection::Mqtt(connection)))
            }
            AstarteConnection::Grpc => {
                let grpc_cfg = self.grpc_config.build()?;
                debug!("parsed Astarte Message Hub config: {:#?}", grpc_cfg);

                let (client, connection) = builder.connect(grpc_cfg).await?.build().await;

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
#[derive(Debug, Default, Deserialize)]
struct MqttConfigBuilder {
    /// Device ID
    device_id: Option<String>,
    /// Astarte realm
    realm: Option<String>,
    /// Device credential
    #[serde(flatten)]
    credential: Option<Credential>,
    /// Astarte pairing url
    pairing_url: Option<String>,
    /// Flag to ignore Astarte SSL errors
    astarte_ignore_ssl: Option<bool>,
}

impl MqttConfigBuilder {
    fn build(self) -> eyre::Result<MqttConfig> {
        let device_id = self.device_id.ok_or_eyre("missing device id")?;
        let realm = self.realm.ok_or_eyre("missing realm")?;
        let credential = self
            .credential
            .ok_or_eyre("missing either a credential secret or a pairing token")?;
        let pairing_url = self.pairing_url.ok_or_eyre("missing pairing url")?;
        // if missing, set the ignore ssl error flat to false
        let astarte_ignore_ssl = self.astarte_ignore_ssl.unwrap_or_default();

        let mut mqtt_cfg = MqttConfig::new(realm, device_id, credential, pairing_url);

        if astarte_ignore_ssl {
            mqtt_cfg.ignore_ssl_errors();
        }

        Ok(mqtt_cfg)
    }
}

/// Config for a gRPC connection to an Astarte Message Hub instance
#[derive(Debug, Default, Deserialize)]
struct GrpcConfigBuilder {
    /// The Endpoint of the Astarte Message Hub
    endpoint: Option<String>,
}

impl GrpcConfigBuilder {
    fn build(self) -> eyre::Result<GrpcConfig> {
        let endpoint = self.endpoint.ok_or_eyre("missing endpoint")?;

        let grpc_cfg = GrpcConfig::from_url(STREAM_RUST_TEST_NODE_UUID, endpoint)
            .wrap_err("failed to create a gRPC config")?;

        Ok(grpc_cfg)
    }
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
