use {
    crate::parser::{
        parse_stream, Block, Context, KConfigError, Located, LocatedBlocks, PeekableChars, PeekableTokenLinesExt,
    },
    std::{fs::File, io::Read, path::Path},
};

/// A parsed KConfig hierarchy.
#[derive(Debug, Default)]
pub struct KConfig {
    /// The blocks found in the hierarchy.
    pub blocks: Vec<Located<Block>>,
}

impl KConfig {
    /// Read a full Kconfig tree starting with the given Kconfig file.
    ///
    /// This recursively reads any configuration files in `source` (or `osource`, `orsource`, `rsource`) statements.
    pub fn parse<F, C>(filename: F, base_dir: &Path, context: &C) -> Result<Self, KConfigError>
    where
        F: AsRef<Path>,
        C: Context,
    {
        let filename = filename.as_ref();
        let mut kconfig = Self::parse_filename(filename, base_dir)?;
        kconfig.resolve_blocks_recursive(base_dir, context)?;
        Ok(kconfig)
    }

    /// Parse the given file.
    pub(crate) fn parse_filename<F>(filename: F, base_dir: &Path) -> Result<Self, KConfigError>
    where
        F: AsRef<Path>,
    {
        let filename = filename.as_ref();
        let mut file = File::open(filename)?;
        let mut input = String::new();
        file.read_to_string(&mut input)?;
        Self::parse_str(PeekableChars::new(input.as_str(), filename.to_string_lossy().as_ref()), base_dir)
    }

    /// Parse a KConfig file from the given string input.
    fn parse_str(input: PeekableChars, base_dir: &Path) -> Result<Self, KConfigError> {
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
        let kconfig = KConfig::parse_str(
            PeekableChars::new(
                r##"mainmenu "Hello, world!"

    source "/tmp/myfile"

    # Read the next file
    source "/tmp/myfile2"
"##,
                "test",
            ),
            Path::new("/tmp"),
        )
        .unwrap();

        assert_eq!(kconfig.blocks.len(), 3);
    }

    #[test]
    fn kconfig_menuconfig() {
        let kconfig = KConfig::parse_str(
            PeekableChars::new(
                r##"
    menuconfig FOO
        bool "Foo"
        default y
        help
          Say foo
"##,
                "test",
            ),
            Path::new("/tmp"),
        )
        .unwrap();

        assert_eq!(kconfig.blocks.len(), 1);
        let Block::MenuConfig(c) = kconfig.blocks[0].as_ref() else {
            panic!("Expected MenuConfig");
        };

        assert_eq!(c.name.as_ref(), "FOO");
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

        let kconfig = match KConfig::parse(kconfig_filename, &base_dir, &context) {
            Ok(kconfig) => kconfig,
            Err(e) => {
                for frame in e.backtrace.frames() {
                    eprintln!("{frame:?}");
                }
                panic!("Failed to parse Kconfig: {}", e);
            }
        };
        assert!(!kconfig.blocks.is_empty());
    }
}
