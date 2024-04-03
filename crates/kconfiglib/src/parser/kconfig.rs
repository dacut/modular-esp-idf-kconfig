use {
    crate::{
        parser::{parse_stream, Block, KConfigError, LocExpr, PeekableChars, PeekableTokenLinesExt},
        Context, ResolveBlock,
    },
    std::{cell::RefCell, fs::File, io::Read, path::Path, rc::Rc},
};

/// A parsed KConfig hierarchy.
#[derive(Debug, Default)]
pub struct KConfig {
    /// The blocks found in the top-level of the KConfig file.
    pub blocks: Vec<Rc<RefCell<Block>>>,
}

impl KConfig {
    /// Read a full Kconfig tree starting with the given Kconfig file.
    ///
    /// This recursively reads any configuration files in `source` (or `osource`, `orsource`, `rsource`) statements.
    pub fn read_from_file<C>(&mut self, filename: &Path, base_dir: &Path, context: &C) -> Result<(), KConfigError>
    where
        C: Context,
    {
        let mut file = File::open(filename)?;
        let mut input = String::new();
        file.read_to_string(&mut input)?;
        self.read_from_str(PeekableChars::new(input.as_str(), filename), base_dir, context)
    }

    /// Populate this KConfig with the tree from the given string input.
    ///
    /// This recursively reads any configuration files in `source` (or `osource`, `orsource`, `rsource`) statements.
    pub fn read_from_str<C>(&mut self, input: PeekableChars, base_dir: &Path, context: &C) -> Result<(), KConfigError>
    where
        C: Context,
    {
        self.read_from_str_raw(input, base_dir, context)?;
        self.resolve_block(base_dir, context, None)?;
        Ok(())
    }

    /// Parse a KConfig file from the given string input without resolving any `source` statements.
    pub(crate) fn read_from_str_raw<C>(&mut self, input: PeekableChars, base_dir: &Path, _context: &C) -> Result<(), KConfigError>
    where
        C: Context,
    {
        let tokens = parse_stream(input)?;
        let mut lines = tokens.peek_lines();

        while let Some(block) = Block::parse(&mut lines, base_dir)? {
            self.blocks.push(Rc::new(RefCell::new(block)));
        }

        Ok(())
    }

    /// Create a new KConfig instance by reading a full Kconfig tree starting with the given Kconfig file.
    ///
    /// This recursively reads any configuration files in `source` (or `osource`, `orsource`, `rsource`) statements.
    pub fn from_file<C>(filename: &Path, base_dir: &Path, context: &C) -> Result<Self, KConfigError>
    where
        C: Context,
    {
        let mut result = Self::default();
        result.read_from_file(filename, base_dir, context)?;
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
        result.read_from_str(input, base_dir, context)?;
        Ok(result)
    }

    /// Parse a KConfig file from the given string input without resolving any `source` statements.
    pub(crate) fn from_str_raw<C>(input: PeekableChars, base_dir: &Path, _context: &C) -> Result<Self, KConfigError>
    where
        C: Context,
    {
        let mut result = Self::default();
        result.read_from_str_raw(input, base_dir, _context)?;
        Ok(result)
    }
    
}

impl ResolveBlock for KConfig {
    type Output = Self;

    fn resolve_block<C>(
        &self,
        base_dir: &Path,
        context: &C,
        parent_cond: Option<&LocExpr>,
    ) -> Result<Self, KConfigError>
    where
        C: Context,
    {
        let blocks = self.blocks.resolve_block(base_dir, context, parent_cond)?;
        let result = Self {
            blocks,
        };

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use {
        crate::parser::{Block, Expr, KConfig, PeekableChars},
        std::{
            collections::HashMap,
            env,
            path::{Path, PathBuf},
        },
    };

    #[test]
    fn kconfig_comments_blank_lines() {
        let context = HashMap::default();

        let kconfig = KConfig::from_str_raw(
            PeekableChars::new(
                r##"mainmenu "Hello, world!"

    source "/tmp/myfile"

    # Read the next file
    source "/tmp/myfile2"
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
        let kconfig = KConfig::from_str_raw(
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
        let Block::MenuConfig(c) = &*kconfig.blocks[0].borrow() else {
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
        let block = kconfig.blocks[1].borrow();
        let bar = block.as_config().unwrap();
        assert_eq!(bar.selects.len(), 1);

        let cfg_target = &bar.selects[0];
        assert_eq!(cfg_target.target_name.as_str(), "BAR");

        let cond = cfg_target.condition.as_ref().unwrap();
        if let Expr::Symbol(sym) = &cond.expr {
            assert_eq!(sym.name.as_str(), "BAZ");
        } else {
            panic!("Expected symbol");
        }
    }
}
