use modular_esp_idf_kconfig_lib::Type;


pub(crate) trait RustIdent {
    /// Return a Rust identifier from this object.
    fn rust_ident(self) -> String;
}

impl RustIdent for &'_ [&'_ str] {
    fn rust_ident(self) -> String {
        let mut result = String::new();
        let mut last_was_digit = false;

        for part in self {
            for (i, c) in part.chars().enumerate() {
                if i == 0 {
                    if last_was_digit && c.is_ascii_digit() {
                        result.push('_');
                    }
                    result.push(c.to_ascii_uppercase());
                } else {
                    result.push(c.to_ascii_lowercase());
                }

                last_was_digit = c.is_ascii_digit();
            }
        }

        result
    }
}

pub(crate) trait RustType {
    /// Return a Rust type from this object.
    fn rust_type(self) -> String;
}

impl RustType for Type {
    fn rust_type(self) -> String {
        match self {
            Type::Bool => "bool".to_string(),
            Type::Int => "i32".to_string(),
            Type::Hex => "u32".to_string(),
            Type::String => "String".to_string(),
            Type::Tristate => "bool".to_string(),
            Type::Unknown => panic!("Unknown type"),
        }
    }
}

impl RustType for &'_ str {
    fn rust_type(self) -> String {
        self.split('_').collect::<Vec<&str>>().rust_ident()
    }
}

pub(crate) trait RustEnumVariant {
    /// Return a Rust enum variant name given this object and the choice it belongs to.
    fn rust_enum_variant(self, choice_name: &str) -> String;
}

impl RustIdent for &'_ str {
    fn rust_ident(self) -> String {
        self.to_lowercase()
    }
}

impl RustEnumVariant for &'_ str {
    fn rust_enum_variant(self, choice_name: &str) -> String {
        let mut config_parts = choice_name.split('_').peekable();
        let mut variant_parts = self.split('_').peekable();

        loop {
            let Some(config_part) = config_parts.peek() else {
                break;
            };
            let Some(variant_part) = variant_parts.peek() else {
                break;
            };

            if *config_part != *variant_part {
                break;
            }

            _ = config_parts.next();
            _ = variant_parts.next();
        }

        if variant_parts.peek().is_none() {
            // We somehow removed all the parts of the variant name.
            if choice_name == "RTC_EXT_CRYST_ADDIT_CURRENT_METHOD" && self == "RTC_EXT_CRYST_ADDIT_CURRENT" {
                // The other variants are None and V2, so make this V1.
                return "V1".to_string();
            }

            panic!("Variant {self} for {choice_name} is the same as/smaller than the config name");
        }

        let result = variant_parts.collect::<Vec<&str>>().rust_ident();
        let first_char = result.chars().next().unwrap();

        if first_char.is_ascii_digit() {
            // let config_name_lower = config_name.to_ascii_lowercase();

            // Enum variants can't start with a digit. Add a prefix to the variant name.
            if choice_name.ends_with("_ANTENNA_INDEX") {
                format!("Antenna{result}")
            } else if choice_name.ends_with("_BOOST") {
                format!("Boost{result}")
            } else if choice_name.ends_with("_CORE_CHOICE") {
                format!("Core{result}")
            } else if choice_name.ends_with("_FREQ")
                || choice_name.ends_with("_FLASHFREQ")
                || choice_name.ends_with("_FREQ_MHZ")
                || matches!(choice_name, "XTAL_FREQ_SEL" | "SPIRAM_SPEED")
            {
                format!("Freq{result}")
            } else if choice_name.ends_with("_INT_LEVEL") || choice_name.ends_with("LVL_SEL") {
                format!("Level{result}")
            } else if choice_name.ends_with("_REV_MIN") {
                format!("Rev{result}")
            } else if choice_name.ends_with("_SIZE") || choice_name.ends_with("_FLASHSIZE") {
                format!("Size{result}")
            } else if choice_name.ends_with("_CORETIMER") {
                format!("Timer{result}")
            } else if result.ends_with("bit") {
                format!("Bits{}", &result[0..result.len() - 3])
            } else {
                panic!("Variant {self} for {choice_name} starts with a digit and cannot deduce a suitable prefix: {result}");
            }
        } else {
            result
        }
    }
}

