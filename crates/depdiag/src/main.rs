//! Generate a GraphViz or Mermaid.js diagram showing the dependencies in Kconfig symbols.
#![allow(dead_code, unused_variables)]

use {
    clap::{builder::PossibleValue, Parser, ValueEnum},
    modular_esp_idf_kconfig_lib::{
        parser::{Block, Choice, Config, Expr, KConfig, LocExpr},
        Target, KCONFIGS_IN, KCONFIGS_PROJBUILD_IN,
    },
    std::{
        cell::RefCell,
        collections::HashMap,
        fmt::{self, Display, Result as FmtResult},
        fs::File,
        io::{stdout, Result as IoResult, Write},
        path::Path,
        rc::Rc,
    },
};

#[derive(Clone, Copy, Debug, Default)]
enum OutputFormat {
    /// Output in GraphViz format.
    #[default]
    GraphViz,

    /// Output in Mermaid.js format.
    Mermaid,
}

impl ValueEnum for OutputFormat {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::GraphViz, Self::Mermaid]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(match self {
            Self::GraphViz => PossibleValue::new("graphviz").alias("GraphViz").help("Output in GraphViz format"),
            Self::Mermaid => PossibleValue::new("mermaid")
                .alias("mermaidjs")
                .alias("mermaid.js")
                .alias("Mermaid")
                .alias("MermaidJS")
                .help("Output in Mermaid.js format"),
        })
    }
}

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

    /// The format to output the diagram in.
    #[arg(long, short, default_value = "graphviz")]
    format: OutputFormat,

    /// The path to the project Kconfigs source file. Defaults to the in-tree version if not present.
    #[arg(long)]
    project_kconfig: Option<String>,

    /// The background color to use for choice nodes.
    #[arg(long, default_value = "#aaffaa")]
    choice_bgcolor: String,

    /// The background color to use for config nodes.
    #[arg(long, default_value = "#ffaaaa")]
    config_bgcolor: String,

    /// The background color to use for menuconfig nodes.
    #[arg(long, default_value = "#ffaaff")]
    menuconfig_bgcolor: String,

    /// The output file to write the diagram to.
    #[arg(long, short, default_value = "-")]
    output: String,

    /// The target to generate the diagram for.
    #[arg(long, short, default_value = "esp32")]
    target: Target,
}

fn main() -> IoResult<()> {
    env_logger::init();
    let mut context = HashMap::<String, String>::default();
    let options = Options::parse();

    context.insert("IDF_PATH".to_string(), options.idf_path.clone());

    if let Some(component_kconfig) = &options.component_kconfig {
        context.insert("COMPONENT_KCONFIGS_SOURCE_FILE".to_string(), component_kconfig.to_string());
    } else {
        context.insert("COMPONENT_KCONFIGS_SOURCE_FILE".to_string(), format!("inline:{KCONFIGS_IN}"));
    }

    if let Some(project_kconfig) = &options.project_kconfig {
        context.insert("COMPONENT_KCONFIGS_PROJBUILD_SOURCE_FILE".to_string(), project_kconfig.to_string());
    } else {
        context
            .insert("COMPONENT_KCONFIGS_PROJBUILD_SOURCE_FILE".to_string(), format!("inline:{KCONFIGS_PROJBUILD_IN}"));
    }

    context.insert("IDF_ENV_FPGA".to_string(), "n".to_string());
    context.insert("IDF_CI_BUILD".to_string(), "n".to_string());

    let base_dir = Path::new(&options.idf_path);
    let kconfig_top = base_dir.join("Kconfig");

    context.insert("IDF_TARGET".to_string(), options.target.name().to_string());
    let kconfig = KConfig::from_file(&kconfig_top, base_dir, &context).unwrap();

    if options.output == "-" {
        write_graph(&mut stdout(), &kconfig, &options)
    } else {
        let mut fd = File::create(&options.output)?;
        write_graph(&mut fd, &kconfig, &options)
    }
}

fn write_graph<W: Write>(writer: &mut W, kconfig: &KConfig, options: &Options) -> IoResult<()> {
    let mut formatter = match options.format {
        OutputFormat::GraphViz => Box::new(GraphVizFormatter { writer, options }) as Box<dyn Formatter>,
        OutputFormat::Mermaid => Box::new(MermaidFormatter { writer, options }) as Box<dyn Formatter>,
    };

    formatter.write_graph(kconfig)
}

struct GraphVizFormatter<'a, 'b, W: Write> {
    options: &'a Options,
    writer: &'b mut W,
}

struct MermaidFormatter<'a, 'b, W: Write> {
    options: &'a Options,
    writer: &'b mut W,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ConfigType {
    Config,
    MenuConfig,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum NodeType {
    Config,
    MenuConfig,
    Choice,
}

impl From<ConfigType> for NodeType {
    fn from(config_type: ConfigType) -> Self {
        match config_type {
            ConfigType::Config => NodeType::Config,
            ConfigType::MenuConfig => NodeType::MenuConfig,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum EdgeType {
    ChoiceAttribute,
    DependsOn,
    Defaults,
    Selects,
}

impl Display for EdgeType {
    fn fmt(&self, f: &mut fmt::Formatter) -> FmtResult {
        f.write_str(match self {
            Self::ChoiceAttribute => "choice attribute",
            Self::DependsOn => "depends on",
            Self::Defaults => "defaults",
            Self::Selects => "selects",
        })
    }
}

trait Formatter {
    fn write_graph_start(&mut self, kconfig: &KConfig) -> IoResult<()>;
    fn write_graph_end(&mut self, kconfig: &KConfig) -> IoResult<()>;

    fn write_node(&mut self, name: &str, node_type: NodeType) -> IoResult<()>;
    fn write_edge(&mut self, source: &str, target: &str, edge_type: EdgeType) -> IoResult<()>;

    fn write_graph(&mut self, kconfig: &KConfig) -> IoResult<()> {
        self.write_graph_start(kconfig)?;
        self.visit_vec(&kconfig.blocks)?;
        self.write_graph_end(kconfig)
    }

    fn visit_vec(&mut self, blocks: &[Rc<RefCell<Block>>]) -> IoResult<()> {
        for block in blocks {
            self.visit_block(block)?;
        }
    
        Ok(())
    }    

    fn visit_block(&mut self, block: &Rc<RefCell<Block>>) -> IoResult<()> {
        match &*block.borrow() {
            Block::Choice(choice) => self.visit_choice(choice),
            Block::Config(config) => self.visit_config(config, ConfigType::Config),
            Block::Menu(menu) => self.visit_vec(&menu.blocks),
            Block::MenuConfig(menu) => self.visit_config(menu, ConfigType::MenuConfig),
            _ => Ok(()),
        }
    }
    
    fn visit_choice(&mut self, choice: &Choice) -> IoResult<()> {
        self.write_node(choice.name.as_str(), NodeType::Choice)?;

        for config in &choice.configs {
            self.visit_config(config, ConfigType::Config)?;
            self.write_edge(config.name.as_str(), choice.name.as_str(), EdgeType::ChoiceAttribute)?;
        }
    
        for dep in choice.depends_on.iter() {
            self.visit_expr( &choice.name, dep, EdgeType::DependsOn)?;
        }
    
        Ok(())
    }        

    fn visit_config(&mut self, config: &Config, config_type: ConfigType) -> IoResult<()> {
        self.write_node(config.name.as_str(), config_type.into())?;
        
        for select in config.selects.iter() {
            self.write_edge(config.name.as_str(), select.target_name.as_str(), EdgeType::Selects)?;
        }
    
        for def in config.defaults.iter() {
            if let Some(cond) = &def.condition {
                self.visit_expr(&config.name, cond, EdgeType::Defaults)?;
            }
        }
    
        for dep in config.depends_on.iter() {
            self.visit_expr(&config.name, dep, EdgeType::DependsOn)?;
        }
    
        Ok(())
    }
    
    fn visit_expr(&mut self, target: &str, expr: &LocExpr, edge_type: EdgeType) -> IoResult<()> {
        match &expr.expr {
            Expr::Symbol(s) => self.write_edge(s.name.as_str(), target, edge_type),
            Expr::Not(e) => self.visit_expr(target, e, edge_type),
            Expr::And(e1, e2) => {
                self.visit_expr(target, e1, edge_type)?;
                self.visit_expr(target, e2, edge_type)
            }
            Expr::Or(e1, e2) => {
                self.visit_expr(target, e1, edge_type)?;
                self.visit_expr(target, e2, edge_type)
            }
            _ => Ok(()),
        }
    }
}

impl<'a, 'b, W: Write> Formatter for GraphVizFormatter<'a, 'b, W> {
    fn write_graph_start(&mut self, kconfig: &KConfig) -> IoResult<()> {
        writeln!(self.writer, r#"digraph "kconfig_dependencies_{}" {{"#, self.options.target.config_name())?;
        writeln!(self.writer, r#"    fontname="Helvetica""#)?;
        writeln!(self.writer, r#"    fontsize="10""#)?;
        writeln!(self.writer, r#"    graph [rankdir=LR]"#)?;
        writeln!(self.writer, r#"    node [shape=box]"#)    
    }

    fn write_graph_end(&mut self, kconfig: &KConfig) -> IoResult<()> {
        writeln!(self.writer, r#"}}"#)
    }

    fn write_node(&mut self, name: &str, node_type: NodeType) -> IoResult<()> {
        let bgcolor = match node_type {
            NodeType::Config => self.options.config_bgcolor.clone(),
            NodeType::MenuConfig => self.options.menuconfig_bgcolor.clone(),
            NodeType::Choice => self.options.choice_bgcolor.clone(),
        };

        writeln!(self.writer, r#"    node [bgcolor="{}"] {}"#, bgcolor, name)
    }

    fn write_edge(&mut self, source: &str, target: &str, edge_type: EdgeType) -> IoResult<()> {
        writeln!(self.writer, r#"    {} -> {} [label="{}"]"#, source, target, edge_type)
    }
}

impl<'a, 'b, W: Write> Formatter for MermaidFormatter<'a, 'b, W> {
    fn write_graph_start(&mut self, kconfig: &KConfig) -> IoResult<()> {
        writeln!(self.writer, "---")?;
        writeln!(self.writer, "title: Kconfig Dependencies for {}", self.options.target.config_name())?;
        writeln!(self.writer, "---")?;
        writeln!(self.writer, "classDiagram")
    }

    fn write_graph_end(&mut self, kconfig: &KConfig) -> IoResult<()> {
        Ok(())
    }

    fn write_node(&mut self, _name: &str, node_type: NodeType) -> IoResult<()> {
        Ok(())
    }

    fn write_edge(&mut self, source: &str, target: &str, edge_type: EdgeType) -> IoResult<()> {
        writeln!(self.writer, r#"    {} <.. {} :{}"#, target, source, edge_type)
    }
}