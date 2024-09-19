// This file is part of Astarte.
//
// Copyright 2024 SECO Mind Srl
//
// SPDX-License-Identifier: Apache-2.0

//! Streaming configuration

use crate::cli::Config;
use crate::math::MathFunction;
use color_eyre::eyre;
use std::f64::consts::PI;
use std::time::SystemTime;
use tracing::debug;

/// Stream configuration
///
/// It represents the actual state of the Stream: the `value` field represents the base value used
///to generate samples to send to Astarte
#[derive(Clone, Debug)]
pub struct StreamConfig {
    /// Astarte interface specifying where data must be sent
    pub(crate) interface: String,
    /// Math function
    pub(crate) math_function: MathFunction,
    /// Scale of the math function
    pub(crate) scale: f64,
    /// Interval between 2 samples
    pub(crate) interval: u64,
    /// Value used to compute samples to send to Astarte
    pub(crate) value: f64,
}

impl StreamConfig {
    /// Init the stream
    pub fn try_from_cli(cli_cfg: Config, now: SystemTime) -> eyre::Result<Self> {
        let initial_value = now.elapsed().map(|t| t.as_secs_f64())?;

        Ok(Self {
            interface: cli_cfg.interface_datastream_do,
            scale: cli_cfg.scale,
            math_function: cli_cfg.math_function,
            interval: cli_cfg.interval_btw_samples,
            value: initial_value,
        })
    }

    /// Update the stream internal configuration
    pub(crate) fn update_cfg(&mut self, update: StreamConfigUpdate) {
        let StreamConfigUpdate { sensor_id, update } = update;

        // TODO: at the moment the sensor_id is only used for debug info. If it is not necessary it
        //  can be removed, otherwise we could add it the the StreamConfig parameter (which means
        //  it should be introduced ad a CLI parameter).

        match update {
            MathConfigUpdate::Function(value) => {
                debug!("update stream math function config with {value} for sensor {sensor_id}");
                self.math_function = value;
            }
            MathConfigUpdate::Interval(value) => {
                debug!("update stream interval config with {value} for sensor {sensor_id}");
                self.interval = value;
            }
            MathConfigUpdate::Scale(value) => {
                debug!("update stream scale config with {value} for sensor {sensor_id}");
                self.scale = value;
            }
        }
    }

    /// retrieve a reference to the Astarte interface
    pub(crate) fn interface(&self) -> &str {
        &self.interface
    }

    /// Compute the next value to send to Astarte
    pub(crate) fn next_value(&self) -> f64 {
        self.math_function.compute(self.value)
    }

    /// Update the inner value
    pub(crate) fn update_value(&mut self) {
        self.value += PI * 2.0 * crate::math::random_interval() * self.scale;
    }
}

/// Stream configuration options
#[derive(Clone, Debug)]
pub struct StreamConfigUpdate {
    /// Sensor id
    pub(crate) sensor_id: String,
    /// math config update
    pub(crate) update: MathConfigUpdate,
}

/// Math configuration options
#[derive(Clone, Debug)]
pub enum MathConfigUpdate {
    /// New math function
    Function(MathFunction),
    /// New interval
    Interval(u64),
    /// New scale
    Scale(f64),
}

impl StreamConfigUpdate {
    pub(crate) fn function<T>(sensor_id: T, function: MathFunction) -> Self
    where
        T: Into<String>,
    {
        Self {
            sensor_id: sensor_id.into(),
            update: MathConfigUpdate::Function(function),
        }
    }

    pub(crate) fn interval<T>(sensor_id: T, interval: u64) -> Self
    where
        T: Into<String>,
    {
        Self {
            sensor_id: sensor_id.into(),
            update: MathConfigUpdate::Interval(interval),
        }
    }

    pub(crate) fn scale<T>(sensor_id: T, scale: f64) -> Self
    where
        T: Into<String>,
    {
        Self {
            sensor_id: sensor_id.into(),
            update: MathConfigUpdate::Scale(scale),
        }
    }
}
