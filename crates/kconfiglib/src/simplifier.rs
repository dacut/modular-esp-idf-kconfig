use {
    crate::{
        parser::{Block, BlockId, Choice, Config, ConfigDefault, Expr, ExprCmpOp, KConfig, Menu, Tristate},
        Context,
    },
    log::*,
    slotmap::SecondaryMap,
    std::collections::HashSet,
};

/// An expression simplifier for KConfig expressions.
#[derive(Debug)]
pub struct Simplifier<'a, 'b, C: Context> {
    /// The KConfig instance to simplify expressions for.
    kconfig: &'a KConfig,

    /// The context to use for simplification.
    #[allow(dead_code)]
    context: &'b C,

    /// The dependency map for the KConfig.
    dep_map: DepMap,

    /// Default values for each config, populated by resolve_block.
    defaults: SecondaryMap<BlockId, Vec<ConfigDefault>>,

    /// Visibility conditions for each config/choice/menu/menuconfig, populated by resolve_block.
    visible_if: SecondaryMap<BlockId, Expr>,

    /// Selected-by conditions for each block, populated by resolve_block.
    selected_by: SecondaryMap<BlockId, Expr>,
}

/// Struct for mapping dependencies in a DAG.
#[derive(Debug, Default)]
pub struct Dep {
    /// The blocks that depend on this block.
    pub depended_on_by: HashSet<BlockId>,

    /// The blocks that this block references on.
    pub depends_on: HashSet<BlockId>,
}

/// A dependency graph of the blocks in a Kconfig file.
#[derive(Debug, Default)]
pub struct DepMap {
    /// Map of blocks to their dependency information.
    pub map: SecondaryMap<BlockId, Dep>,

    /// Blocks with no dependencies.
    pub roots: HashSet<BlockId>,
}

/// The type returned from [`Simplifier::choices()`].
pub type ChoiceFilter<'a> = std::iter::FilterMap<slotmap::basic::Values<'a, BlockId, Block>, for<'b> fn(&'b Block) -> Option<&'b Choice>>;

/// The type returned from [`Simplifier::configs()`].
pub type ConfigFilter<'a> = std::iter::FilterMap<slotmap::basic::Values<'a, BlockId, Block>, for<'b> fn(&'b Block) -> Option<&'b Config>>;

impl<'a, 'b, C: Context> Simplifier<'a, 'b, C> {
    /// Create a new Simplifier from the given KConfig and context.
    pub fn new(kconfig: &'a KConfig, context: &'b C) -> Self {
        let dep_map = DepMap::from(kconfig);
        Self {
            kconfig,
            context,
            dep_map,
            defaults: SecondaryMap::new(),
            visible_if: SecondaryMap::new(),
            selected_by: SecondaryMap::new(),
        }
    }

    /// Resolve all expressions in the KConfig.
    pub fn resolve(&mut self) {
        let roots = self.dep_map.roots.clone();
        for root_id in roots {
            self.resolve_block(root_id)
        }
    }

    /// Return all choices in the KConfig.
    pub fn choices(&self) -> ChoiceFilter {
        self.kconfig.blocks.values().filter_map(Block::as_choice)
    }

    /// Return all configs in the KConfig.
    pub fn configs(&self) -> ConfigFilter {
        self.kconfig.blocks.values().filter_map(Block::as_config_or_menuconfig)
    }

    /// Returns the block with the given id.
    pub fn get_block(&self, id: BlockId) -> Option<&Block> {
        self.kconfig.blocks.get(id)
    }
    
    /// Resolve a block, recursively resolving all blocks that depend solely on it.
    ///
    /// For each dependent block that depends on this block:
    /// * The dependency is removed.
    /// * If the dependent block has no other dependencies, `resolve_block()` is invoked recursively.
    fn resolve_block(&mut self, block_id: BlockId) {
        let dep = self.dep_map.map.get(block_id).unwrap();
        let dependents = dep.depended_on_by.clone();

        // At this point, all dependencies should have been resolved.
        assert!(dep.depends_on.is_empty());

        let block = self.kconfig.blocks.get(block_id).unwrap();
        match block {
            Block::Config(c) => {
                self.resolve_config_defaults(block_id, c);
                self.resolve_config_selects(c);
            }
            Block::Menu(m) => self.resolve_menu(block_id, m),
            Block::MenuConfig(c) => {
                self.resolve_config_defaults(block_id, c);
                self.resolve_config_selects(c);
            }
            _ => (),
        }

        // For each block that depends on this block, remove the dependency.
        for dependent_id in dependents {
            let dependent_dep = self.dep_map.map.get_mut(dependent_id).unwrap();
            dependent_dep.depends_on.remove(&block_id);
        }
    }

    /// Evaluate expressions in the defaults for a config.
    ///
    /// Any symbols in the expressions are resolved to their block ids.
    fn resolve_config_defaults(&mut self, block_id: BlockId, config: &Config) {
        let mut defaults = vec![];

        for default in config.defaults.iter() {
            let value = self.resolve_expr(&default.value);
            let condition = self.resolve_expr(&default.condition);
            defaults.push(ConfigDefault {
                value,
                condition,
            });
        }

        let defaults = simplify_defaults(defaults);
        let old = self.defaults.insert(block_id, defaults);
        assert!(old.is_none());
    }

    /// Propagate selects values to the dependent blocks.
    ///
    /// `resolve_config_default` must have been called on this Config before calling this method.
    fn resolve_config_selects(&mut self, config: &Config) {
        for select in config.selects.iter() {
            let target_name = &select.target_name;

            if let Some(target_id) = self.kconfig.configs.get(target_name) {
                let config_expr =
                    Expr::and(self.resolve_expr(&Expr::Symbol(config.name.to_string())), select.condition.clone());

                // The target exists. Update its selected_by expression.
                self.selected_by
                    .entry(*target_id)
                    .unwrap()
                    .and_modify(|expr| *expr = Expr::or(expr.clone(), config_expr.clone()))
                    .or_insert(config_expr);
            } else {
                // The target does not exist.
                info!("Config {} selects unknown target {}", config.name, target_name);
            }
        }
    }

    /// Resolve a menu block.
    fn resolve_menu(&mut self, _block_id: BlockId, _menu: &Menu) {
        todo!()
    }

    /// Resolve an expression.
    ///
    /// This requires that all of the symbols in the expression have been resolved already.
    pub fn resolve_expr(&self, expr: &Expr) -> Expr {
        match expr {
            // Static values.
            Expr::Tristate(_) | Expr::Hex(_) | Expr::Int(_) | Expr::String(_) => expr.clone(),
            Expr::Symbol(s) => {
                if let Some(block_id) = self.kconfig.configs.get(s) {
                    // If this symbol is potentially visible, we can't simplify it further since it's user-configurable.
                    let visible_if = self.visible_if.get(*block_id);
                    if visible_if.is_none() || visible_if == Some(&Expr::Tristate(Tristate::True)) {
                        // Not user-configurable.
                        let defaults = &self.defaults.get(*block_id);
                        assert!(defaults.is_none() || !defaults.unwrap().is_empty());

                        if let Some(sel_expr) = self.selected_by.get(*block_id) {
                            if defaults.is_none() {
                                // If there are defaults and a selected-by, we can't simplify it.
                                expr.clone()
                            } else {
                                // No defaults; return the selected-by expression for this symbol.
                                sel_expr.clone()
                            }
                        } else {
                            match defaults {
                                None => {
                                    // No defaults; this is always false.
                                    Expr::Tristate(Tristate::False)
                                }
                                Some(defaults) if defaults.len() == 1 => {
                                    // Single default; return the value if the condition is always true.
                                    let default = &defaults[0];
                                    if default.condition == Expr::Tristate(Tristate::True) {
                                        default.value.clone()
                                    } else {
                                        // Condition is not always true; return the symbol itself.
                                        expr.clone()
                                    }
                                }
                                _ => {
                                    // Multiple defaults; return the symbol itself.
                                    expr.clone()
                                }
                            }
                        }
                    } else {
                        // User-configurable, so we can't statically determine the value.
                        // Return the symbol itself.
                        expr.clone()
                    }
                } else {
                    // Config does not exist and can never be set.
                    Expr::Tristate(Tristate::False)
                }
            }
            Expr::And(lhs, rhs) => {
                let lhs = self.resolve_expr(lhs);
                let rhs = self.resolve_expr(rhs);
                Expr::and(lhs, rhs)
            }
            Expr::Or(lhs, rhs) => {
                let lhs = self.resolve_expr(lhs);
                let rhs = self.resolve_expr(rhs);
                Expr::or(lhs, rhs)
            }
            Expr::Not(inner) => {
                let inner = self.resolve_expr(inner);
                match inner {
                    Expr::Tristate(t) => Expr::Tristate(!t),
                    Expr::Not(v) => *v,
                    _ => Expr::Not(inner.into()),
                }
            }
            Expr::Cmp(op, lhs, rhs) => self.simplify_cmp(*op, lhs, rhs),
        }
    }

    fn simplify_cmp(&self, op: ExprCmpOp, lhs: &Expr, rhs: &Expr) -> Expr {
        let lhs = self.resolve_expr(lhs);
        let rhs = self.resolve_expr(rhs);

        match lhs {
            Expr::Tristate(lhs_v) => match rhs {
                Expr::Tristate(rhs_v) => match op {
                    ExprCmpOp::Eq => Expr::Tristate((lhs_v == rhs_v).into()),
                    ExprCmpOp::Ne => Expr::Tristate((lhs_v != rhs_v).into()),
                    _ => panic!("Invalid comparison of tristate values: {op}"),
                },
                Expr::Hex(_) | Expr::Int(_) | Expr::String(_) => {
                    panic!("Invalid comparison of tristate and non-tristate values: {lhs:?}, {rhs:?}")
                }
                _ => Expr::Cmp(op, lhs.into(), rhs.into()),
            },
            Expr::Int(lhs_v) => match rhs {
                Expr::Int(rhs_v) => match op {
                    ExprCmpOp::Eq => Expr::Tristate((lhs_v == rhs_v).into()),
                    ExprCmpOp::Ne => Expr::Tristate((lhs_v != rhs_v).into()),
                    ExprCmpOp::Lt => Expr::Tristate((lhs_v < rhs_v).into()),
                    ExprCmpOp::Le => Expr::Tristate((lhs_v <= rhs_v).into()),
                    ExprCmpOp::Gt => Expr::Tristate((lhs_v > rhs_v).into()),
                    ExprCmpOp::Ge => Expr::Tristate((lhs_v >= rhs_v).into()),
                },
                Expr::Hex(_) | Expr::Tristate(_) | Expr::String(_) => {
                    panic!("Invalid comparison of int and non-int values: {lhs:?}, {rhs:?}")
                }
                _ => Expr::Cmp(op, lhs.into(), rhs.into()),
            },
            Expr::Hex(lhs_v) => match rhs {
                Expr::Hex(rhs_v) => match op {
                    ExprCmpOp::Eq => Expr::Tristate((lhs_v == rhs_v).into()),
                    ExprCmpOp::Ne => Expr::Tristate((lhs_v != rhs_v).into()),
                    ExprCmpOp::Lt => Expr::Tristate((lhs_v < rhs_v).into()),
                    ExprCmpOp::Le => Expr::Tristate((lhs_v <= rhs_v).into()),
                    ExprCmpOp::Gt => Expr::Tristate((lhs_v > rhs_v).into()),
                    ExprCmpOp::Ge => Expr::Tristate((lhs_v >= rhs_v).into()),
                },
                Expr::Int(_) | Expr::Tristate(_) | Expr::String(_) => {
                    panic!("Invalid comparison of hex and non-hex values: {lhs:?}, {rhs:?}")
                }
                _ => Expr::Cmp(op, lhs.into(), rhs.into()),
            },
            Expr::String(ref lhs_v) => match rhs {
                Expr::String(ref rhs_v) => match op {
                    ExprCmpOp::Eq => Expr::Tristate((lhs_v == rhs_v).into()),
                    ExprCmpOp::Ne => Expr::Tristate((lhs_v != rhs_v).into()),
                    _ => panic!("Invalid comparison of string values: {op}"),
                },
                Expr::Hex(_) | Expr::Int(_) | Expr::Tristate(_) => {
                    panic!("Invalid comparison of string and non-string values: {lhs:?}, {rhs:?}")
                }
                _ => Expr::Cmp(op, lhs.into(), rhs.into()),
            },
            _ => Expr::Cmp(op, lhs.into(), rhs.into()),
        }
    }
}

/// Collapse adjacent defaults with the same value.
fn simplify_defaults(defaults: Vec<ConfigDefault>) -> Vec<ConfigDefault> {
    let mut result = Vec::with_capacity(defaults.len());
    let mut last_value = None;

    for default in defaults {
        if last_value.is_none() {
            last_value = Some(default.value.clone());
            result.push(default);
            continue;
        } else if last_value.as_ref() == Some(&default.value) {
            // Collapse this default into the last one.
            let last_default = result.last_mut().unwrap();
            last_default.condition = Expr::or(last_default.condition.clone(), default.condition);
        } else {
            last_value = Some(default.value.clone());
            result.push(default);
        }
    }

    result
}

impl From<&KConfig> for DepMap {
    /// Create a new DepMap from a KConfig.
    fn from(kconfig: &KConfig) -> Self {
        let mut this = Self::default();
        this.populate_from(kconfig);
        this
    }
}

impl DepMap {
    fn populate_from(&mut self, kconfig: &KConfig) {
        for (block_id, block) in kconfig.blocks.iter() {
            match block {
                Block::Config(c) => {
                    // Each block selected by this block depends on this block.
                    for select in c.selects.iter() {
                        let Some(dependent_id) = kconfig.configs.get(&select.target_name) else {
                            info!("Config {} selects unknown target {}", c.name, select.target_name);
                            continue;
                        };

                        self.create_dep(block_id, *dependent_id);
                    }

                    // This block depends on anything found in depends_on.
                    for dep in c.depends_on.symbols() {
                        let Some(dependency_id) = kconfig.configs.get(&dep) else {
                            info!("Config {} depends on unknown target {}", c.name, dep);
                            continue;
                        };

                        self.create_dep(*dependency_id, block_id);
                    }

                    // And if this has a parent menu, it depends on that.
                    if let Some(parent) = c.parent {
                        self.create_dep(parent, block_id);
                    }
                }
                Block::Menu(m) => {
                    // This menu depends on the symbols found in the depends_on condition.
                    for dependency in m.depends_on.symbols() {
                        let Some(dependency_id) = kconfig.configs.get(&dependency) else {
                            info!("Menu {} depends on unknown symbol {}", m.prompt, dependency);
                            continue;
                        };

                        self.create_dep(*dependency_id, block_id);
                    }

                    // And the symbols in the `visible if` condition.
                    for symbol in m.visibility.symbols() {
                        let Some(symbol_id) = kconfig.configs.get(&symbol) else {
                            info!("Menu {} visibility depends on unknown symbol {}", m.prompt, symbol);
                            continue;
                        };

                        self.create_dep(*symbol_id, block_id);
                    }

                    // And if this has a parent menu, it depends on that.
                    if let Some(parent) = m.parent {
                        self.create_dep(parent, block_id);
                    }
                }
                Block::Choice(c) => {
                    // This config depends on all of the symbols found in the default conditions.
                    for default in c.defaults.iter() {
                        for source in default.condition.symbols() {
                            let Some(source_id) = kconfig.configs.get(&source) else {
                                info!("Choice {} default depends on unknown symbol {}", c.name, source);
                                continue;
                            };

                            self.create_dep(*source_id, block_id);
                        }
                    }

                    // And all of the symbols found in the depends on condition.
                    for dependency in c.depends_on.symbols() {
                        let Some(dependency_id) = kconfig.configs.get(&dependency) else {
                            info!("Choice {} depends on unknown symbol {}", c.name, dependency);
                            continue;
                        };

                        self.create_dep(*dependency_id, block_id);
                    }

                    // And if this has a parent menu, it depends on that.
                    if let Some(parent) = c.parent {
                        self.create_dep(parent, block_id);
                    }
                }
                _ => {}
            }
        }
    }

    /// Create a dependency from `dependency` to `dependent`.
    ///
    /// * The [`Dep`] instance for `dependency` will have `dependent` added to its `depended_on_by` set.
    /// * The [`Dep`] instance for `dependent` will have `dependency` added to its `depends_on` set.
    fn create_dep(&mut self, dependency: BlockId, dependent: BlockId) {
        // Track that `dependency`` has a
        self.map
            .entry(dependency)
            .unwrap()
            .or_insert_with(|| {
                // If this is the first time we've seen this dependency, add it to the roots.
                // We know it has no dependencies yet.
                self.roots.insert(dependency);
                Dep::default()
            })
            .depended_on_by
            .insert(dependent);

        self.map.entry(dependent).unwrap().or_default().depends_on.insert(dependent);

        // Always remove the dependent from the roots if it was there.
        self.roots.remove(&dependent);
    }
}


