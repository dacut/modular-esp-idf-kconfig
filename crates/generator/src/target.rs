use std::fmt::{Display, Formatter, Result as FmtResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Target {
    Esp32,
    Esp32c2,
    Esp32c3,
    Esp32c6,
    Esp32h2,
    Esp32s2,
    Esp32s3,
}

impl Display for Target {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            Self::Esp32 => write!(f, "esp32"),
            Self::Esp32c2 => write!(f, "esp32c2"),
            Self::Esp32c3 => write!(f, "esp32c3"),
            Self::Esp32c6 => write!(f, "esp32c6"),
            Self::Esp32h2 => write!(f, "esp32h2"),
            Self::Esp32s2 => write!(f, "esp32s2"),
            Self::Esp32s3 => write!(f, "esp32s3"),
        }
    }
}

impl Target {
    pub(crate) fn all() -> &'static [Self] {
        &[Self::Esp32, Self::Esp32c2, Self::Esp32c3, Self::Esp32c6, Self::Esp32h2, Self::Esp32s2, Self::Esp32s3]
    }

    pub(crate) fn sdkconfig(self) -> &'static str {
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

    pub(crate) fn name(self) -> &'static str {
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
