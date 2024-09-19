// This file is part of Astarte.
//
// Copyright 2024 SECO Mind Srl
//
// SPDX-License-Identifier: Apache-2.0

use std::env;
use astarte_device_sdk::builder::{DeviceBuilder, DeviceSdkBuild};
use astarte_device_sdk::transport::mqtt::MqttConfig;
use astarte_device_sdk::EventLoop;
use clap::Parser;
use color_eyre::eyre;
use std::env::VarError;
use std::time::SystemTime;
use stream_rust_test::astarte::{send_data, AstarteConfigBuilder};
use stream_rust_test::cli::Config;
use tokio::task::JoinSet;
use tracing::{debug, error};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

const DEVICE_DATASTREAM: &str =
    include_str!("../interfaces/org.astarte-platform.genericsensors.Values.json");

fn env_filter() -> eyre::Result<EnvFilter> {
    let filter = env::var("RUST_LOG").or_else(|err| match err {
        VarError::NotPresent => Ok("stream_rust_test=debug".to_string()),
        err @ VarError::NotUnicode(_) => Err(err),
    })?;

    let env_filter = EnvFilter::try_new(filter)?;

    Ok(env_filter)
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let filter = env_filter()?;
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();

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
