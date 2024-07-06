use {
    crate::{
        parser::{parse_stream, Block, BlockId, Expr, KConfigError, PeekableChars, PeekableTokenLinesExt, Tristate},
        Context,
    },
    slotmap::SlotMap,
    std::{collections::HashMap, fs::File, io::Read, path::Path},
};

/// A parsed KConfig hierarchy.
#[derive(Debug, Default)]
pub struct KConfig {
    /// The blocks found in the the KConfig file.
    pub blocks: SlotMap<BlockId, Block>,

    /// Map of config symbol names to their block ids.
    pub configs: HashMap<String, BlockId>,
}

impl KConfig {
    /// Read a full Kconfig tree starting with the given Kconfig file.
    ///
    /// This recursively reads any configuration files in `source` (or `osource`, `orsource`, `rsource`) statements.
    pub fn read_from_file<C>(
        &mut self,
        filename: &Path,
        base_dir: &Path,
        parent_condition: Expr,
        parent: Option<BlockId>,
        context: &C,
    ) -> Result<Vec<BlockId>, KConfigError>
    where
        C: Context,
    {
        let mut file = File::open(filename)?;
        let mut input = String::new();
        file.read_to_string(&mut input)?;
        self.read_from_str(PeekableChars::new(input.as_str(), filename), base_dir, parent_condition, parent, context)
    }

    /// Populate this KConfig with the tree from the given string input.
    ///
    /// This recursively reads any configuration files in `source` (or `osource`, `orsource`, `rsource`) statements.
    pub fn read_from_str<C>(
        &mut self,
        input: PeekableChars,
        base_dir: &Path,
        parent_condition: Expr,
        parent: Option<BlockId>,
        context: &C,
    ) -> Result<Vec<BlockId>, KConfigError>
    where
        C: Context,
    {
        let tokens = parse_stream(input)?;
        let mut lines = tokens.peek_lines();

        Block::parse_blocks(self, &mut lines, base_dir, parent_condition, parent, context)
    }

    /// Create a new KConfig instance by reading a full Kconfig tree starting with the given Kconfig file.
    ///
    /// This recursively reads any configuration files in `source` (or `osource`, `orsource`, `rsource`) statements.
    pub fn from_file<C>(filename: &Path, base_dir: &Path, context: &C) -> Result<Self, KConfigError>
    where
        C: Context,
    {
        let mut result = Self::default();
        result.read_from_file(filename, base_dir, Expr::Tristate(Tristate::True), None, context)?;
        Ok(result)
    }

    /// Create a new KConfig with the tree from the given string input.
    ///
    /// This recursively reads any configuration files in `source` (or `osource`, `orsource`, `rsource`) statements.
    pub fn from_str<C>(input: PeekableChars, base_dir: &Path, context: &C) -> Result<Self, KConfigError>
    where
        C: Context,
    {
        let mut result = Self::default();
        result.read_from_str(input, base_dir, Expr::Tristate(Tristate::True), None, context)?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use {
        crate::parser::{Expr, KConfig, PeekableChars, Tristate},
        std::{
            collections::HashMap,
            env,
            path::{Path, PathBuf},
        },
    };

    #[test]
    fn kconfig_comments_blank_lines() {
        let context = HashMap::default();

        let kconfig = KConfig::from_str(
            PeekableChars::new(
                r##"mainmenu "Hello, world!"

    config FOO

    # Another config
    config BAR
"##,
                Path::new("test"),
            ),
            Path::new("/tmp"),
            &context,
        )
        .unwrap();

        assert_eq!(kconfig.blocks.len(), 3);
    }

    #[test]
    fn kconfig_menuconfig() {
        let context = HashMap::default();
        let kconfig = KConfig::from_str(
            PeekableChars::new(
                r##"
    menuconfig FOO
        bool "Foo"
        default y
        help
          Say foo
"##,
                Path::new("test"),
            ),
            Path::new("/tmp"),
            &context,
        )
        .unwrap();

        assert_eq!(kconfig.blocks.len(), 1);

        for (_, block) in kconfig.blocks.iter() {
            let Some(c) = block.as_menuconfig() else {
                panic!("Expected MenuConfig");
            };

            assert_eq!(c.name.as_str(), "FOO");
        }
    }

    #[test_log::test]
    fn esp_idf() {
        let mut context = HashMap::default();
        let base_dir = PathBuf::from(
            env::var("CARGO_MANIFEST_DIR")
                .unwrap_or_else(|_| env::current_dir().unwrap().to_str().unwrap().to_string()),
        );
        let esp_idf = base_dir.join("tests/esp-idf");
        let kconfig_filename = esp_idf.join("Kconfig");

        context.insert("IDF_PATH".to_string(), esp_idf.to_str().unwrap().to_string());
        context.insert("IDF_TARGET".to_string(), "esp32".to_string());
        context.insert(
            "COMPONENT_KCONFIGS_SOURCE_FILE".to_string(),
            esp_idf.join("Kconfigs.in").to_str().unwrap().to_string(),
        );
        context.insert(
            "COMPONENT_KCONFIGS_PROJBUILD_SOURCE_FILE".to_string(),
            esp_idf.join("Kconfigs.projbuild.in").to_str().unwrap().to_string(),
        );

        let kconfig = KConfig::from_file(&kconfig_filename, &base_dir, &context).unwrap();
        assert!(!kconfig.blocks.is_empty());
    }

    #[test_log::test]
    fn config_selects() {
        let context = HashMap::default();

        let kconfig = KConfig::from_str(
            PeekableChars::new(
                r##"config FOO
    default n

config BAR
    default y
    select BAR if BAZ

config BAZ
    default y"##,
                Path::new("test"),
            ),
            Path::new("/tmp"),
            &context,
        )
        .unwrap();

        assert_eq!(kconfig.blocks.len(), 3);

        let mut foo_seen = false;
        let mut bar_seen = false;
        let mut baz_seen = false;

        for (_, block) in kconfig.blocks.iter() {
            let Some(c) = block.as_config() else {
                panic!("Expected Config");
            };

            match c.name.as_str() {
                "FOO" => {
                    foo_seen = true;
                    assert_eq!(c.defaults.len(), 1);
                    assert_eq!(c.defaults[0].value, Expr::Tristate(Tristate::False));
                }
                "BAR" => {
                    bar_seen = true;
                    assert_eq!(c.defaults.len(), 1);
                    assert_eq!(c.defaults[0].value, Expr::Tristate(Tristate::True));
                    assert_eq!(c.selects.len(), 1);
                    assert_eq!(c.selects[0].target_name.as_str(), "BAR");
                    assert_eq!(c.selects[0].condition, Expr::Symbol("BAZ".to_string()));
                }
                "BAZ" => {
                    baz_seen = true;
                    assert_eq!(c.defaults.len(), 1);
                    assert_eq!(c.defaults[0].value, Expr::Tristate(Tristate::True));
                }
                _ => {
                    unreachable!("Unexpected config {}", c.name.as_str());
                }
            }
        }

        assert!(foo_seen);
        assert!(bar_seen);
        assert!(baz_seen);
    }
}
