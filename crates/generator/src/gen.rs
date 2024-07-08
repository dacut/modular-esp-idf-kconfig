#[allow(unused_imports)]
use {
    crate::{RustEnumVariant, RustIdent, RustType, Target},
    modular_esp_idf_kconfig_lib::{
        Block, BlockId, Choice, Config, Context, Expr, GetLocation, KConfig, Menu, Simplifier, Tristate,
    },
    std::{
        collections::hash_map::Entry,
        collections::{HashMap, HashSet},
        fmt::Debug,
        io::{Result as IoResult, Write},
    },
};

#[derive(Debug)]
pub(crate) struct Generator<'a, 'b, C: Context> {
    pub target: Target,
    pub simplifier: Simplifier<'a, 'b, C>,
}

impl<'a, 'b, C: Context> Generator<'a, 'b, C> {
    pub fn new(target: Target, simplifier: Simplifier<'a, 'b, C>) -> Self {
        Self {
            target,
            simplifier,
        }
    }

    pub fn write<W>(&self, w: &mut W) -> IoResult<()>
    where
        W: Write,
    {
        writeln!(w, "//! SDK configuration for {}", self.target.name())?;
        writeln!(w)?;
        writeln!(w, "use {{")?;
        writeln!(w, "    crate::HeaderGen,")?;
        writeln!(w, "    serde::{{Deserialize, Serialize}},")?;
        writeln!(w, "    std::{{")?;
        writeln!(w, "        fmt::{{Display, Formatter, Result as FmtResult}},")?;
        writeln!(w, "        io::{{Result as IoResult, Write}},")?;
        writeln!(w, "    }},")?;
        writeln!(w, "}};")?;
        writeln!(w)?;

        writeln!(w, "#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]")?;
        writeln!(w, "#[serde(rename_all = \"PascalCase\")]")?;
        writeln!(w, "pub struct {} {{", self.target.sdkconfig())?;

        self.write_fields(w)?;

        writeln!(w, "}}")?;

        self.write_sdkconfig_impl(w)?;
        self.write_choice_enums(w)?;

        Ok(())
    }

    // Write the fields for the SDK configuration.
    fn write_fields<W>(&self, w: &mut W) -> IoResult<()>
    where
        W: Write,
    {
        // Write all the choice fields first.
        for choice in self.simplifier.choices() {
            self.write_choice_field(w, choice)?;
        }

        for config in self.simplifier.configs() {
            self.write_config_field(w, config)?;
        }

        Ok(())
    }

    // Write a user-configurable choice field for the SDK configuration struct.
    fn write_choice_field<W>(&self, w: &mut W, choice: &Choice) -> IoResult<()>
    where
        W: Write,
    {
        let rust_type = choice.name.as_ref().rust_type();
        writeln!(w)?;
        writeln!(w, "    /// {} (write_choice_field)", choice.name)?;

        write_help(w, choice.help.as_ref().map(|s| s.as_ref().as_str()), "    ")?;
        writeln!(w, "    pub {}: Option<{}>,", choice.name.as_ref().rust_ident(), rust_type)?;

        Ok(())
    }

    // Write a user-configurable config field for the SDK configuration struct.
    fn write_config_field<W>(&self, w: &mut W, config: &Config) -> IoResult<()>
    where
        W: Write,
    {
        let rust_config_name = config.name.as_ref().rust_ident();
        let config_type = config.r#type.rust_type();

        writeln!(w)?;
        writeln!(w, "    /// {} (write_config_field)", config.name)?;

        if let Some(help) = &config.help {
            if !help.inner.is_empty() {
                writeln!(w, "    ///")?;
                for line in help.inner.trim().split('\n') {
                    writeln!(w, "    /// {}", line)?;
                }
            }
        }

        writeln!(w, "    pub {}: Option<{}>,", rust_config_name, config_type)?;

        Ok(())
    }

    // Write the impl block for the SDK configuration.
    fn write_sdkconfig_impl<W>(&self, w: &mut W) -> IoResult<()>
    where
        W: Write,
    {
        writeln!(w)?;
        writeln!(w, "impl {} {{", self.target.sdkconfig())?;

        // Write all the choice fields first.
        for choice in self.simplifier.choices() {
            self.write_choice_accessor(w, choice)?;
        }

        // Then write all the config fields.
        for config in self.simplifier.configs() {
            self.write_config_accessor(w, config)?;
        }

        writeln!(w, "}}")?;

        Ok(())
    }

    fn write_choice_enums<W>(&self, w: &mut W) -> IoResult<()>
    where
        W: Write,
    {
        for choice in self.simplifier.choices() {
            self.write_choice_enum(w, choice)?;
        }

        self.write_sdkconfig_header_generator(w)?;

        Ok(())
    }

    fn write_choice_enum<W>(&self, w: &mut W, choice: &Choice) -> IoResult<()>
    where
        W: Write,
    {
        let rust_type = choice.name.inner.rust_type();
        writeln!(w)?;

        writeln!(w, "/// {} (write_choice_enum)", choice.name)?;
        write_help(w, choice.help.as_ref().map(|h| &h.inner), "")?;
        writeln!(w, "#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]")?;
        writeln!(w, "pub enum {rust_type} {{")?;
        for (i, config_id) in choice.configs.iter().enumerate() {
            if i > 0 {
                writeln!(w)?;
            }

            let Some(config_block) = self.simplifier.get_block(*config_id) else {
                panic!("Config block not found for choice variant: {config_id:?}");
            };

            let Some(config) = config_block.as_config() else {
                panic!("Block {config_id:?} is not a config: {config_block:?}");
            };

            writeln!(w, "    /// {}", config.name)?;

            write_help(w, config.help.as_ref().map(|h| &h.inner), "    ")?;

            let rust_variant_name = config.name.inner.rust_enum_variant(choice.name.as_ref());
            writeln!(w, "    {rust_variant_name},")?;
        }

        writeln!(w, "}}")?;
        writeln!(w)?;
        writeln!(w, "impl Display for {rust_type} {{")?;
        writeln!(w, "    fn fmt(&self, f: &mut Formatter) -> FmtResult {{")?;
        writeln!(w, "        match self {{")?;
        for block_id in choice.configs.iter() {
            let Some(config_block) = self.simplifier.get_block(*block_id) else {
                panic!("Config block not found for choice variant: {block_id:?}");
            };

            let Some(config) = config_block.as_config() else {
                panic!("Block {block_id:?} is not a config: {config_block:?}");
            };

            let rust_variant_name = config.name.inner.rust_enum_variant(choice.name.as_ref());
            writeln!(w, "            Self::{rust_variant_name} => write!(f, \"{}\"),", config.name)?;
        }
        writeln!(w, "        }}")?;
        writeln!(w, "    }}")?;
        writeln!(w, "}}")?;

        Ok(())
    }

    fn write_sdkconfig_header_generator<W>(&self, w: &mut W) -> IoResult<()>
    where
        W: Write,
    {
        writeln!(w)?;
        writeln!(w, "impl HeaderGen for {} {{", self.target.sdkconfig())?;
        writeln!(w, "    fn write_header<W: Write>(&self, w: &mut W) -> IoResult<()> {{")?;

        // Write all the choice fields first.
        for choice in self.simplifier.choices() {
            self.write_sdkconfig_choice(w, choice)?;
        }

        // Then write all the config fields.
        for config in self.simplifier.configs() {
            self.write_sdkconfig_config(w, config)?;
        }

        writeln!(w, "    }}")?;
        writeln!(w, "}}")?;
        Ok(())
    }

    fn write_choice_accessor<W>(&self, w: &mut W, choice: &Choice) -> IoResult<()>
    where
        W: Write,
    {
        let rust_ident = choice.name.inner.rust_ident();
        let rust_type = choice.name.inner.rust_type();

        writeln!(w)?;
        writeln!(w, "    /// Return the value of the {} choice, finding a default if it is not set.", choice.name)?;
        writeln!(w, "    pub fn {rust_ident}(&self) -> {rust_type} {{")?;
        writeln!(w, "        if let Some(value) = self.{rust_ident} {{")?;
        writeln!(w, "            value")?;

        let n_defaults = choice.defaults.len();
        for (i, default) in choice.defaults.iter().enumerate() {
            let enum_variant = default.target.inner.rust_enum_variant(choice.name.as_ref());
            if default.condition == Expr::Tristate(Tristate::True) || i == n_defaults - 1 {
                writeln!(w, "        }} else {{")?;
                writeln!(w, "            {rust_type}::{enum_variant}")?;
                break;
            }

            writeln!(w, "        }} else if {} {{", rustify_expr(&default.condition))?;
            writeln!(w, "            {rust_type}::{enum_variant}")?;
        }
        writeln!(w, "        }}")?;
        writeln!(w, "    }}")?;
        Ok(())
    }

    fn write_config_accessor<W>(&self, w: &mut W, config: &Config) -> IoResult<()>
    where
        W: Write,
    {
        let rust_ident = config.name.inner.rust_ident();
        let rust_type = config.r#type.rust_type();

        writeln!(w)?;
        writeln!(w, "    /// Return the value of the {} config, finding a default if it is not set.", config.name)?;
        writeln!(w, "    pub fn {rust_ident}(&self) -> {rust_type} {{")?;
        writeln!(w, "        if let Some(value) = self.{rust_ident} {{")?;
        writeln!(w, "            value")?;

        for (i, default) in config.defaults.iter().enumerate() {
            if default.condition == Expr::Tristate(Tristate::True) || i == config.defaults.len() - 1 {
                writeln!(w, "        }} else {{")?;
                writeln!(w, "            {}", rustify_expr(&default.value))?;
                break;
            }

            writeln!(w, "        }} else if {} {{", rustify_expr(&default.condition))?;
            writeln!(w, "            {}", rustify_expr(&default.value))?;
        }
        writeln!(w, "        }}")?;
        writeln!(w, "    }}")?;
        Ok(())
    }

    fn write_sdkconfig_choice<W>(&self, w: &mut W, choice: &Choice) -> IoResult<()>
    where
        W: Write,
    {
        let rust_ident = choice.name.inner.rust_ident();
        writeln!(w)?;
        writeln!(w, "        if let Some(value) = self.{rust_ident} {{")?;
        writeln!(w, r##"            writeln!(w, "#define {{value}}")?;"##)?;

        if choice.defaults.is_empty() {
            writeln!(w, "        }}")?;
            return Ok(());
        }

        let n_defaults = choice.defaults.len();

        for (i, default) in choice.defaults.iter().enumerate() {
            if default.condition == Expr::Tristate(Tristate::True) || i == n_defaults - 1 {
                writeln!(w, r"        }} else {{")?;
                writeln!(w, r##"            writeln!(w, "#define {}")?;"##, default.target)?;
                break;
            }

            writeln!(w, "        }} else if {} {{", rustify_expr(&default.condition))?;
            writeln!(w, r##"            writeln!(w, "#define {}")?;"##, default.target)?;
        }
        writeln!(w, "        }}")?;
        Ok(())
    }

    fn write_sdkconfig_config<W>(&self, w: &mut W, config: &Config) -> IoResult<()>
    where
        W: Write,
    {
        let rust_ident = config.name.inner.rust_ident();
        writeln!(w)?;
        writeln!(w, "        if let Some(value) = self.{rust_ident} {{")?;
        writeln!(w, r##"            writeln!(w, "#define {{value}}")?;"##)?;
        for default in config.defaults.iter() {
            writeln!(w, "        }} else if {} {{", rustify_expr(&default.condition))?;
            writeln!(w, r##"            writeln!(w, "#define {}")?;"##, config.name)?;
        }
        writeln!(w, "        }}")?;
        Ok(())
    }
}

fn write_help<W, S>(w: &mut W, help: Option<S>, indent: &str) -> IoResult<()>
where
    S: AsRef<str>,
    W: Write,
{
    let Some(help) = help else {
        return Ok(());
    };

    let help_str = help.as_ref().trim();
    if !help_str.is_empty() {
        writeln!(w, "{indent}///")?;
        for line in help_str.split('\n') {
            writeln!(w, "{indent}/// {}", line)?;
        }
    }

    Ok(())
}

fn rustify_expr(expr: &Expr) -> String {
    match expr {
        Expr::Tristate(tristate) => match tristate {
            Tristate::True => "true".to_string(),
            _ => "false".to_string(),
        },
        Expr::Symbol(symbol) => format!("self.{}()", symbol.rust_ident()),
        Expr::Hex(value) => format!("0x{:x}", value),
        Expr::Int(value) => value.to_string(),
        Expr::String(value) => format!("{value:?}"),
        Expr::Cmp(op, lhs, rhs) => format!("{} {} {}", rustify_expr(lhs), op, rustify_expr(rhs)),
        Expr::Not(expr) => format!("!{}", rustify_expr(expr)),
        Expr::And(lhs, rhs) => format!("{} && {}", rustify_expr(lhs), rustify_expr(rhs)),
        Expr::Or(lhs, rhs) => format!("{} || {}", rustify_expr(lhs), rustify_expr(rhs)),
    }
}
