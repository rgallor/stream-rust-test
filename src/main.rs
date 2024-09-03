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
use color_eyre::eyre::{bail, eyre};
use serde::Deserialize;
use std::path::PathBuf;
use std::time::SystemTime;
use stream_rust_test::math::{BaseValue, MathFunction};
use tokio::task::JoinSet;
use tokio_stream::wrappers::ReadDirStream;
use tokio_stream::StreamExt;
use tracing::{debug, error};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, Deserialize)]
struct AstarteConfig {
    /// Astarte realm
    realm: String,
    /// Device ID
    device_id: String,
    /// Device credential secret
    #[serde(default)]
    credentials_secret: Option<String>,
    /// Device pairing token
    #[serde(default)]
    pairing_token: Option<String>,
    /// Astarte pairing url
    pairing_url: String,
    /// Astarte store directory
    store_directory: PathBuf,
    /// Flag to ignore Astarte SSL errors
    astarte_ignore_ssl: bool,
    /// Path to folder containing the Astarte Device interfaces
    interfaces_directory: PathBuf,
}

/// Configuration for the values to be sent to Astarte
#[derive(Debug, Clone, Parser)]
#[clap(version, about)]
struct Config {
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
    let astarte_cfg: AstarteConfig = load_astarte_cfg().await?;

    debug!("Parsed Astarte config: {:#?}", astarte_cfg);

    // define type of credential (pairing token or credential secret) to use to establish an MQTT
    // connection with Astarte
    let cred = if let Some(pairing) = astarte_cfg.pairing_token.as_deref() {
        Credential::paring_token(pairing)
    } else if let Some(secret) = astarte_cfg.credentials_secret.as_deref() {
        Credential::secret(secret)
    } else {
        bail!("missing credential secret or pairing token");
    };

    // define MQTT configuration options
    let mut mqtt_config = MqttConfig::new(
        astarte_cfg.realm,
        astarte_cfg.device_id,
        cred,
        astarte_cfg.pairing_url,
    );

    if astarte_cfg.astarte_ignore_ssl {
        mqtt_config.ignore_ssl_errors();
    }

    // connect to Astarte
    let (client, mut connection) = DeviceBuilder::new()
        .store_dir(astarte_cfg.store_directory.as_path())
        .await?
        .interface_directory(astarte_cfg.interfaces_directory.as_path())?
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

async fn load_astarte_cfg() -> eyre::Result<AstarteConfig> {
    // search the astarte-device-DEVICE_ID_HERE-conf with the DEVICE_ID specified by the user
    // starting from the root of the project
    let dirs = tokio::fs::read_dir(".").await?;
    let dirs_stream = ReadDirStream::new(dirs);

    let mut dirs = dirs_stream
        .filter_map(|res| res.ok().map(|e| e.path()))
        .filter(|path| {
            if !path.is_dir() {
                return false;
            }

            let name = path
                .file_name()
                .expect("failed to retrieve the folder name")
                .to_string_lossy();

            // true if the folder name starts and ends with the predefined values
            name.starts_with("astarte-device-") && name.ends_with("-conf")
        })
        .collect::<Vec<_>>()
        .await;

    // if more folders are present, take only the first one
    let Some(dir) = dirs.first_mut() else {
        return Err(eyre!("No astarte devices config folder found"));
    };

    dir.push("config.toml");

    let file = tokio::fs::read_to_string(dir).await?;

    // retrieve the astarte config information
    let astarte_cfg: AstarteConfig = toml::from_str(&file)?;

    Ok(astarte_cfg)
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
