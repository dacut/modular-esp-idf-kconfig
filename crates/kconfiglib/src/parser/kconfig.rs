use {
    crate::parser::{parse_stream, Block, Context, KConfigError, LocatedBlocks, PeekableChars, PeekableTokenLinesExt},
    std::{fs::File, io::Read, path::Path},
};

/// A parsed KConfig hierarchy.
#[derive(Debug, Default)]
pub struct KConfig {
    /// The blocks found in the hierarchy.
    pub blocks: Vec<Block>,
}

impl KConfig {
    /// Read a full Kconfig tree starting with the given Kconfig file.
    ///
    /// This recursively reads any configuration files in `source` (or `osource`, `orsource`, `rsource`) statements.
    pub fn parse<C>(filename: &Path, base_dir: &Path, context: &C) -> Result<Self, KConfigError>
    where
        C: Context,
    {
        Self::parse_filename(filename, base_dir, context)
    }

    /// Parse the given file.
    pub fn parse_filename<C>(filename: &Path, base_dir: &Path, context: &C) -> Result<Self, KConfigError>
    where
        C: Context,
    {
        let mut file = File::open(filename)?;
        let mut input = String::new();
        file.read_to_string(&mut input)?;
        Self::parse_str(PeekableChars::new(input.as_str(), filename), base_dir, context)
    }

    /// Parse a KConfig file from the given string input.
    pub fn parse_str<C>(input: PeekableChars, base_dir: &Path, context: &C) -> Result<Self, KConfigError>
    where
        C: Context,
    {
        let mut kconfig = Self::parse_str_raw(input, base_dir)?;
        kconfig.resolve_blocks_recursive(base_dir, context)?;

        Ok(kconfig)
    }

    /// Parse a KConfig file from the given string input without resolving any `source` statements.
    pub(crate) fn parse_str_raw(input: PeekableChars, base_dir: &Path) -> Result<Self, KConfigError> {
        let tokens = parse_stream(input)?;
        let mut lines = tokens.peek_lines();
        let mut blocks = Vec::new();

        while let Some(block) = Block::parse(&mut lines, base_dir)? {
            blocks.push(block);
        }

        Ok(Self {
            blocks,
        })
    }
}

impl LocatedBlocks for KConfig {
    fn resolve_blocks_recursive<C>(&mut self, base_dir: &Path, context: &C) -> Result<(), KConfigError>
    where
        C: Context,
    {
        self.blocks.resolve_blocks_recursive(base_dir, context)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::{Block, KConfig, PeekableChars},
        std::{
            collections::HashMap,
            env,
            path::{Path, PathBuf},
        },
    };

    #[test]
    fn kconfig_comments_blank_lines() {
        let kconfig = KConfig::parse_str_raw(
            PeekableChars::new(
                r##"mainmenu "Hello, world!"

    source "/tmp/myfile"

    # Read the next file
    source "/tmp/myfile2"
"##,
                Path::new("test"),
            ),
            Path::new("/tmp"),
        )
        .unwrap();

        assert_eq!(kconfig.blocks.len(), 3);
    }

    #[test]
    fn kconfig_menuconfig() {
        let kconfig = KConfig::parse_str_raw(
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
        )
        .unwrap();

        assert_eq!(kconfig.blocks.len(), 1);
        let Block::MenuConfig(c) = &kconfig.blocks[0] else {
            panic!("Expected MenuConfig");
        };

        assert_eq!(c.name.as_str(), "FOO");
    }

    #[test_log::test]
    fn esp_idf() {
        let mut context = HashMap::default();
        let base_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
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

        let kconfig = KConfig::parse(&kconfig_filename, &base_dir, &context).unwrap();
        assert!(!kconfig.blocks.is_empty());
    }
}
