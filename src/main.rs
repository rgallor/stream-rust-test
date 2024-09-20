// This file is part of Astarte.
//
// Copyright 2024 SECO Mind Srl
//
// SPDX-License-Identifier: Apache-2.0

use astarte_device_sdk::builder::{DeviceBuilder, DeviceSdkBuild};
use astarte_device_sdk::store::SqliteStore;
use astarte_device_sdk::transport::mqtt::{Credential, MqttConfig};
use astarte_device_sdk::{Client, DeviceClient, EventLoop};
use clap::Parser;
use color_eyre::eyre;
use color_eyre::eyre::OptionExt;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use std::{env, io};
use stream_rust_test::math::{BaseValue, MathFunction};
use tokio::task::JoinSet;
use tracing::{debug, error};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

const DEVICE_DATASTREAM: &str =
    include_str!("../interfaces/org.astarte-platform.genericsensors.Values.json");

#[derive(Debug, Default, Deserialize)]
struct AstarteConfigBuilder {
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
    // init astarte config from env var if set
    fn from_env() -> Self {
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
    async fn update_with_toml(&mut self, path: impl AsRef<Path>) {
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
    fn build(self) -> eyre::Result<AstarteConfig> {
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

#[derive(Debug)]
struct AstarteConfig {
    /// Astarte realm
    realm: String,
    /// Device ID
    device_id: String,
    /// Device credential
    credential: Credential,
    /// Astarte pairing url
    pairing_url: String,
    /// Astarte store directory
    store_directory: PathBuf,
    /// Flag to ignore Astarte SSL errors
    astarte_ignore_ssl: bool,
}

/// Configuration for the values to be sent to Astarte
#[derive(Debug, Clone, Parser)]
#[clap(version, about)]
struct Config {
    /// Path to the directory containing the Astarte configuration file config.toml
    ///
    /// First, the Astarte configuration is taken from ENV vars, then from the config.toml if the
    /// path has been specified
    #[clap(short, long, env = "ASTARTE_CONFIG_PATH")]
    astarte_config_path: Option<PathBuf>,
    /// Math function the device will use to send data to Astarte
    #[clap(short, long, default_value = "default", env = "MATH_FUNCTION")]
    math_function: MathFunction,
    /// Interface name to send data to
    #[clap(
        long,
        default_value = "org.astarte-platform.genericsensors.Values",
        env = "INTERFACE_NAME"
    )]
    interface_datastream_do: String,
    /// Milliseconds the device must wait before sending data to Astarte
    #[clap(short, long, default_value = "1000", env = "INTERVAL_BTW_SAMPLES")]
    interval_btw_samples: u64,
    /// Scale for the generation of the data to send
    #[clap(short, long, default_value = "1.0", env = "SCALE")]
    scale: f64,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(EnvFilter::from_default_env())
        .try_init()?;

    // time instant when the program starts its execution
    let now = SystemTime::now();

    // initialize CLI configuration options
    let cli_cfg = Config::parse();

    debug!("Parsed config: {:#?}", cli_cfg);

    // Load astarte configuration
    let mut astarte_cfg_builder = AstarteConfigBuilder::from_env();

    if let Some(path) = &cli_cfg.astarte_config_path {
        let path = path.join("config.toml");
        astarte_cfg_builder.update_with_toml(path).await;
    }

    let astarte_cfg = astarte_cfg_builder.build()?;

    debug!("Parsed Astarte config: {:#?}", astarte_cfg);

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
    let (client, mut connection) = DeviceBuilder::new()
        .store_dir(astarte_cfg.store_directory)
        .await?
        .interface_str(DEVICE_DATASTREAM)?
        .connect(mqtt_config)
        .await?
        .build();

    let mut tasks = JoinSet::<eyre::Result<()>>::new();

    // spawn task to poll updates from the MQTT connection with Astarte
    tasks.spawn(async move { connection.handle_events().await.map_err(Into::into) });

    // spawn task to send data to Astarte
    tasks.spawn(send_data(client, now, cli_cfg));

    // handle tasks termination
    while let Some(res) = tasks.join_next().await {
        match res {
            Ok(Ok(())) => {}
            Err(err) if err.is_cancelled() => {}
            Err(err) => {
                error!(error = %err, "Task panicked");
                return Err(err.into());
            }
            Ok(Err(err)) => {
                error!(error = %err, "Task returned an error");
                return Err(err);
            }
        }
    }

    Ok(())
}

/// Send data to Astarte
async fn send_data(
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
