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
use color_eyre::eyre::bail;
use std::path::PathBuf;
use std::time::SystemTime;
use stream_rust_test::math::{BaseValue, MathFunction};
use tokio::task::JoinSet;
use tracing::{debug, error};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// Astarte device configuration.
#[derive(Debug, Clone, Parser)]
#[clap(version, about)]
pub struct Config {
    /// Astarte realm
    #[clap(long, env = "REALM")]
    pub realm: String,
    /// Device ID
    #[clap(long, env = "DEVICE_ID")]
    pub device_id: String,
    /// Device credential secret
    #[clap(long, env = "CREDENTIALS_SECRET")]
    pub credentials_secret: Option<String>,
    /// Device pairing token
    #[clap(long, env = "PAIRING_TOKEN")]
    pub pairing_token: Option<String>,
    /// Astarte pairing url
    #[clap(long, env = "PAIRING_URL")]
    pub pairing_url: String,
    /// Astarte store directory
    #[clap(long, env = "STORE_DIR")]
    pub store_dir: String,
    /// Flag to ignore Astarte SSL errors
    #[clap(long, default_value = "true", env = "IGNORE_SSL_ERRORS")]
    pub ignore_ssl_errors: bool,
    /// Path to folder containing the Astarte Device interfaces
    #[clap(long, default_value = PathBuf::from("interfaces").into_os_string())]
    pub interfaces_folder: PathBuf,
    /// Math function the device will use to send data to Astarte
    #[clap(long, default_value = "default", env = "MATH_FUNCTION")]
    pub math_function: MathFunction,
    /// Interface name to send data to
    #[clap(
        long,
        default_value = "org.astarte-platform.genericsensors.Values",
        env = "INTERFACE_NAME"
    )]
    pub interface_datastream_do: String,
    /// Milliseconds the device must wait before sending data to Astarte
    #[clap(long, default_value = "1000", env = "INTERVAL_BTW_SAMPLES")]
    pub interval_btw_samples: u64,
    /// Scale for the generation of the data to send
    #[clap(long, default_value = "1.0", env = "SCALE")]
    pub scale: f64,
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

    // initialize configuration options
    let cfg = Config::parse();

    debug!("Parsed config: {:#?}", cfg);

    // define type of credential (pairing token or credential secret) to use to establish an MQTT
    // connection with Astarte
    let cred = if let Some(secret) = cfg.credentials_secret.as_deref() {
        Credential::secret(secret)
    } else if let Some(pairing) = cfg.pairing_token.as_deref() {
        Credential::paring_token(pairing)
    } else {
        bail!("missing credential secret or pairing token");
    };

    // define MQTT configuration options
    let mut mqtt_config = MqttConfig::new(cfg.realm, cfg.device_id, cred, cfg.pairing_url);

    if cfg.ignore_ssl_errors {
        mqtt_config.ignore_ssl_errors();
    }

    // connect to Astarte
    let (client, mut connection) = DeviceBuilder::new()
        .store_dir(cfg.store_dir)
        .await?
        .interface_directory(cfg.interfaces_folder.as_path())?
        .connect(mqtt_config)
        .await?
        .build();

    let mut tasks = JoinSet::<eyre::Result<()>>::new();

    // spawn task to poll updates from the MQTT connection with Astarte
    tasks.spawn(async move { connection.handle_events().await.map_err(Into::into) });

    // spawn task to send data to Astarte
    tasks.spawn(send_data(
        client,
        now,
        cfg.math_function,
        cfg.interface_datastream_do,
        cfg.interval_btw_samples,
        cfg.scale,
    ));

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
    math_function: MathFunction,
    interface_datastream_do: String,
    interval_btw_samples: u64,
    scale: f64,
) -> eyre::Result<()> {
    let mut base_value = BaseValue::try_from_system_time(now, scale)?;

    debug!("sending data to Astarte with {math_function} math function");

    loop {
        // Send data to Astarte
        let value = math_function.compute(base_value.value());

        client
            .send(&interface_datastream_do, "/test/value", value)
            .await?;

        debug!("data sent on endpoint /test/value, content: {value}");

        // update the data to send at the next iteration
        base_value.update();

        // Sleep interval secs
        tokio::time::sleep(std::time::Duration::from_millis(interval_btw_samples)).await;
    }
}
