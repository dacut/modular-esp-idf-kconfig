use std::fmt::{Display, Formatter, Result as FmtResult};

/// ESP32 targets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target {
    /// ESP32
    Esp32,

    /// ESP32-C2
    Esp32c2,

    /// ESP32-C3
    Esp32c3,

    /// ESP32-C6
    Esp32c6,

    /// ESP32-H2
    Esp32h2,

    /// ESP32-S2
    Esp32s2,

    /// ESP32-S3
    Esp32s3,
}

impl Display for Target {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        f.write_str(self.config_name())
    }
}

impl Target {
    /// Return all targets as a slice.
    pub fn all() -> &'static [Self] {
        &[Self::Esp32, Self::Esp32c2, Self::Esp32c3, Self::Esp32c6, Self::Esp32h2, Self::Esp32s2, Self::Esp32s3]
    }

    /// Return the name of the SdkConfig struct for this target.
    pub fn sdkconfig(self) -> &'static str {
        match self {
            Self::Esp32 => "SdkConfigEsp32",
            Self::Esp32c2 => "SdkConfigEsp32c2",
            Self::Esp32c3 => "SdkConfigEsp32c3",
            Self::Esp32c6 => "SdkConfigEsp32c6",
            Self::Esp32h2 => "SdkConfigEsp32h2",
            Self::Esp32s2 => "SdkConfigEsp32s2",
            Self::Esp32s3 => "SdkConfigEsp32s3",
        }
    }

    /// Return the configuration name for this target as a str.
    pub fn config_name(self) -> &'static str {
        match self {
            Self::Esp32 => "esp32",
            Self::Esp32c2 => "esp32c2",
            Self::Esp32c3 => "esp32c3",
            Self::Esp32c6 => "esp32c6",
            Self::Esp32h2 => "esp32h2",
            Self::Esp32s2 => "esp32s2",
            Self::Esp32s3 => "esp32s3",
        }
    }

    /// Return the name of the target as a str.
    pub fn name(self) -> &'static str {
        match self {
            Self::Esp32 => "ESP32",
            Self::Esp32c2 => "ESP32C2",
            Self::Esp32c3 => "ESP32C3",
            Self::Esp32c6 => "ESP32C6",
            Self::Esp32h2 => "ESP32H2",
            Self::Esp32s2 => "ESP32S2",
            Self::Esp32s3 => "ESP32S3",
        }
    }
}

#[cfg(feature = "clap")]
impl clap::ValueEnum for Target {
    fn value_variants<'a>() -> &'a [Self] {
        Self::all()
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(
            clap::builder::PossibleValue::new(self.config_name())
            .alias(self.name())
            .help(format!("Use {} as the target MCU", self.config_name()))
        )
    }
}