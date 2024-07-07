//! Generate sdkconfig.rs files from an ESP-IDF source code hierarchy.
#![allow(dead_code, unused_variables)]

mod gen;
mod strutil;
mod target;

pub(crate) use crate::{gen::*, strutil::*, target::*};

use {
    clap::Parser,
    modular_esp_idf_kconfig_lib::{KConfig, Simplifier},
    std::{collections::HashMap, fs::File, io::Result as IoResult, path::Path},
};

/// Command line options for the generator.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Options {
    /// The path to the ESP-IDF tree.
    #[arg(long, env = "IDF_PATH")]
    idf_path: String,

    /// The path to the component Kconfigs source file. Defaults to the in-tree version if not present.
    #[arg(long)]
    component_kconfig: Option<String>,

    /// The path to the project Kconfigs source file. Defaults to the in-tree version if not present.
    #[arg(long)]
    project_kconfig: Option<String>,

    /// The output directory to write generated files to.
    #[arg(long, short, default_value = ".")]
    output_dir: String,
}

/// KConfigs.in for `COMPONENT_KCONFIGS_SOURCE_FILE`.
pub(crate) const KCONFIGS_IN: &str = include_str!("Kconfigs.in");

/// KConfigs.projbuild.in for `COMPONENT_KCONFIGS_PROJBUILD_SOURCE_FILE`.
pub(crate) const KCONFIGS_PROJBUILD_IN: &str = include_str!("Kconfigs.projbuild.in");

/// The string `"COMPONENT_KCONFIGS_SOURCE_FILE"`
pub(crate) const COMPONENT_KCONFIGS_SOURCE_FILE: &str = "COMPONENT_KCONFIGS_SOURCE_FILE";

/// The string `"COMPONENT_KCONFIGS_PROJBUILD_SOURCE_FILE"`
pub(crate) const COMPONENT_KCONFIGS_PROJBUILD_SOURCE_FILE: &str = "COMPONENT_KCONFIGS_PROJBUILD_SOURCE_FILE";

/// The string `"IDF_CI_BUILD"`
pub(crate) const IDF_CI_BUILD: &str = "IDF_CI_BUILD";

/// The string `"IDF_ENV_FPGA"`
pub(crate) const IDF_ENV_FPGA: &str = "IDF_ENV_FPGA";

/// The string `"IDF_PATH"`
pub(crate) const IDF_PATH: &str = "IDF_PATH";

/// The string `"IDF_TOOLCHAIN"`
pub(crate) const IDF_TOOLCHAIN: &str = "IDF_TOOLCHAIN";

/// The string `"clang"`, for use with IDF_TOOLCHAIN.
pub(crate) const IDF_TOOLCHAIN_CLANG: &str = "clang";

fn main() -> IoResult<()> {
    env_logger::init();
    let mut context = HashMap::<String, String>::default();
    let options = Options::parse();

    context.insert(IDF_PATH.to_string(), options.idf_path.clone());
    context.insert(IDF_TOOLCHAIN.to_string(), IDF_TOOLCHAIN_CLANG.to_string());
    context.insert(
        COMPONENT_KCONFIGS_SOURCE_FILE.to_string(),
        if let Some(component_kconfig) = &options.component_kconfig {
            component_kconfig.to_string()
        } else {
            format!("inline:{KCONFIGS_IN}")
        },
    );
    context.insert(
        COMPONENT_KCONFIGS_PROJBUILD_SOURCE_FILE.to_string(),
        if let Some(project_kconfig) = &options.project_kconfig {
            project_kconfig.to_string()
        } else {
            format!("inline:{KCONFIGS_PROJBUILD_IN}")
        },
    );

    context.insert(IDF_ENV_FPGA.to_string(), "n".to_string());
    context.insert(IDF_CI_BUILD.to_string(), "n".to_string());

    let base_dir = Path::new(&options.idf_path);
    let kconfig_top = base_dir.join("Kconfig");

    for target in Target::all() {
        context.insert("IDF_TARGET".to_string(), target.to_string().to_uppercase());
        let kconfig = KConfig::from_file(&kconfig_top, base_dir, &context).unwrap();
        let mut simplifier = Simplifier::new(&kconfig, &context);
        simplifier.resolve();
        let gen = Generator::new(*target, simplifier);

        let output_dir = Path::new(&options.output_dir);
        let filename = output_dir.join(format!("{target}.rs"));
        let mut output = File::create(filename)?;

        gen.write(&mut output)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screaming_snake_variants() {
        let result = "CONFIG_FOO_BAR".rust_enum_variant("CONFIG_FOO");
        assert_eq!(result, "Bar");

        let result = "PARTITION_TABLE_SINGLE_APP".rust_enum_variant("PARTITION_TABLE_TYPE");
        assert_eq!(result, "SingleApp");
    }

    #[test]
    fn screaming_snake_digits() {
        let result = "V_1_8".rust_ident();
        assert_eq!(result, "v_1_8");

        let result = "1_8V".rust_ident();
        assert_eq!(result, "1_8v");
    }
}
