use {
    crate::resolve::{
        RcResChoice, RcResConfig, RcResMenu, ResChoice, ResChoiceDefault, ResChoiceMap, ResConfig,
        ResConfigChoiceParent, ResConfigDefault, ResConfigMap, ResConfigRange, ResExpr, ResLitValue, ResMenu,
        ResMenuVec,
    },
    modular_esp_idf_kconfig_lib::parser::{Block, Choice, Config, Context, Menu},
    std::{cell::RefCell, rc::Rc},
};

/// Maps for choices and configs.
#[derive(Debug, Default)]
pub(crate) struct SymbolMap {
    /// Map of choice symbols.
    pub choices: ResChoiceMap,

    /// Map of config symbols.
    pub configs: ResConfigMap,

    /// The first level menus.
    pub menus: ResMenuVec,
}

impl SymbolMap {
    pub fn get_choice(&self, name: &str) -> Option<RcResChoice> {
        self.choices.get(name).cloned()
    }

    pub fn get_config(&self, name: &str) -> Option<RcResConfig> {
        self.configs.get(name).cloned()
    }

    /// Update this `SymbolMap` with resolved [`ResChoice`] and [`ResConfig`] entries from the specified list of
    /// blocks.
    ///
    /// # Parameters
    /// * `blocks` - The blocks to convert into resolved entries.
    /// * `context` - The context to use for resolving expressions.
    pub(crate) fn resolve_symbols<C>(&mut self, blocks: Vec<Block>, context: &C)
    where
        C: Context,
    {
        // First, create an a ResSymbol entry for each symbol found in the blocks.
        self.intermediate_resolve_symbols(&blocks, context);

        // Deduplicate symbols between choices and configs.
        self.deduplicate_choice_configs();

        // Finally, update the dependencies for each config.
        self.final_resolve_symbols(blocks, context);
    }

    /// Create a [`ResChoice`] or [`ResConfig`] entry for each symbol in the blocks.
    pub(crate) fn intermediate_resolve_symbols<C>(&mut self, blocks: &[Block], context: &C)
    where
        C: Context,
    {
        for block in blocks {
            match block {
                Block::Choice(choice) => {
                    self.intermediate_resolve_choice(choice);
                }
                Block::Config(config) => {
                    self.intermediate_resolve_config(config);
                }
                Block::If(_) => panic!("If blocks must be resolved at this point."),
                Block::Mainmenu(_) => (),
                Block::Menu(menu) => {
                    self.intermediate_resolve_menu(menu, context);
                }
                Block::MenuConfig(mc) => {
                    self.intermediate_resolve_config(mc);
                }
                Block::Source(_) => panic!("Source blocks must be resolved at this point."),
            }
        }
    }

    /// Create a [`ResChoice`] entry for the specified [`Choice`], then create a [`ResConfig`] entry for each config under
    /// the `Choice`.
    pub(crate) fn intermediate_resolve_choice(&mut self, choice: &Choice) -> RcResChoice {
        // Create a ResChoice entry for this choice.
        let res_choice = Rc::new(RefCell::new(ResChoice::from_choice(choice)));

        self.choices.insert(choice.name.to_string(), res_choice.clone());
        let weak_res_choice = Rc::downgrade(&res_choice);

        // Then do the same for each config under this choice.
        for config in &choice.configs {
            let res_config = self.intermediate_resolve_config(config);

            // And set the parent of the config to the choice.
            res_config.borrow_mut().parent =
                Some(ResConfigChoiceParent::new(choice.name.as_ref(), weak_res_choice.clone()).into());
        }

        res_choice
    }

    /// Create a [`ResConfig`] entry for the specified [`Config`].
    pub(crate) fn intermediate_resolve_config(&mut self, config: &Config) -> RcResConfig {
        log::info!("Adding config {name:?}: {config:?}", name = config.name);

        let res_config = Rc::new(RefCell::new(ResConfig::from_config(config)));

        self.configs.insert(config.name.to_string(), res_config.clone());
        res_config
    }

    /// Create a [`ResMenu`] entry for the specified [`Menu`].
    fn intermediate_resolve_menu<C>(&mut self, menu: &Menu, context: &C) -> RcResMenu
    where
        C: Context,
    {
        let res_menu = Rc::new(RefCell::new(ResMenu::from_menu(menu, self, context)));
        self.menus.push(res_menu.clone());
        res_menu.borrow_mut().intermediate_resolve_symbols(&menu.blocks, self, context);
        res_menu
    }

    /// Fix symbols duplicated between choices and configs.
    fn deduplicate_choice_configs(&mut self) {
        for (name, choice) in self.choices.iter() {
            // If there's a config with the same name, remove it.
            let Some(config) = self.configs.remove(name) else {
                continue;
            };

            // The config should have statements like:
            // * `default 1 if choice_config_1`
            // * `default 2 if choice_config_2`

            // These values are moved into the choice.

            // Make sure we have the same number of defaults in the config as configs in the choice.
            assert_eq!(config.borrow().defaults.len(), choice.borrow().configs.len());

            let mut r#type = None;

            for default in config.borrow().defaults.iter() {
                let value = match &default.value {
                    ResExpr::Bool(b) => ResLitValue::Bool(*b),
                    ResExpr::Hex(h) => ResLitValue::Hex(*h),
                    ResExpr::Integer(i) => ResLitValue::Integer(*i),
                    ResExpr::String(s) => ResLitValue::String(s.clone()),
                    _ => panic!("Unexpected default value in config"),
                };

                if let Some(r#type) = r#type {
                    assert_eq!(
                        r#type,
                        value.r#type(),
                        "Types of defaults in choice/config alias do not match? {:?} vs {:?}",
                        r#type,
                        value.r#type()
                    );
                } else {
                    r#type = Some(value.r#type());
                }

                let ResExpr::Symbol((_, cc)) = &default.condition else {
                    panic!("Expected default in choice/config alias to be a symbol: {:?}", default.condition)
                };

                // Set the value on this choice's config.
                cc.borrow_mut().parent.as_mut().unwrap().as_choice_mut().unwrap().value = Some(value);
            }
        }
    }

    /// Update dependencies for each [`ResChoice`] and [`ResConfig`] entry.
    fn final_resolve_symbols<C>(&mut self, blocks: Vec<Block>, context: &C)
    where
        C: Context,
    {
        for block in blocks {
            match block {
                Block::Choice(choice) => self.final_resolve_choice(choice, context),
                Block::Config(config) => self.final_resolve_config(config, context),
                Block::If(_) => panic!("If blocks must be resolved at this point."),
                Block::Mainmenu(_) => (),
                Block::Menu(menu) => self.final_resolve_symbols(menu.blocks, context),
                Block::MenuConfig(mc) => self.final_resolve_config(mc, context),
                Block::Source(_) => panic!("Source blocks must be resolved at this point."),
            }
        }

        // Try to evaluate the value of each config.
        for (name, config) in self.configs.iter() {
            config.borrow_mut().resolve(name, self, context);
        }
    }

    /// Update dependencies for the specified [`Choice`].
    #[allow(clippy::useless_asref)] // https://github.com/rust-lang/rust-clippy/issues/12135
    fn final_resolve_choice<C>(&mut self, choice: Choice, context: &C)
    where
        C: Context,
    {
        let res_choice = self.get_choice(choice.name.as_ref()).unwrap();

        // Convert each config symbol in the choice to the tuple of (name, RcResConfig).
        for config in choice.configs.into_iter() {
            let res_config = self.get_config(config.name.as_ref()).unwrap();

            res_choice.borrow_mut().configs.push((config.name.to_string(), res_config));
        }

        // Convert each default value.
        for default in choice.defaults.into_iter() {
            let target_name = default.target.to_string();
            let target = self.get_config(&target_name).unwrap();

            let condition = if let Some(cond) = default.condition {
                ResExpr::new(self, cond, context)
            } else {
                ResExpr::Bool(true)
            };

            res_choice.borrow_mut().defaults.push(ResChoiceDefault {
                target_name,
                target,
                condition,
            });
        }

        // Convert each dependency.
        let mut deps = ResExpr::Bool(true);
        for dep in choice.depends_on.into_iter() {
            let dep = ResExpr::new(self, dep, context);
            deps &= dep;
        }

        res_choice.borrow_mut().depends_on = deps;
    }

    /// Update dependencies for the specified [`Config`].
    #[allow(clippy::useless_asref)] // https://github.com/rust-lang/rust-clippy/issues/12135
    fn final_resolve_config<C>(&mut self, config: Config, context: &C)
    where
        C: Context,
    {
        assert!(config.implies.is_empty(), "Implies not yet supported.");

        let name = config.name.as_str();
        if name == "BOOTLOADER_SKIP_VALIDATE_IN_DEEP_SLEEP" || name == "SOC_RTC_FAST_MEM_SUPPORTED" {
            log::trace!("Found {name}");
        }

        // If this config duplicates a choice name, skip it.
        if self.choices.contains_key(config.name.as_str()) {
            // Sanity check: make sure this config doesn't select anything or have any ranges.
            assert!(config.selects.is_empty(), "Config with same name as choice cannot have selects");
            assert!(config.ranges.is_empty(), "Config with same name as choice cannot have ranges");
            return;
        }

        let res_config = self.get_config(config.name.as_ref()).unwrap();

        for select in config.selects.into_iter() {
            let target_name = select.target_name.to_string();
            let Some(target) = self.get_config(&target_name) else {
                log::info!("Config {} selects non-existent target {}", config.name, target_name);
                continue;
            };

            let config_expr = ResExpr::Symbol((target_name, res_config.clone()));

            let cond = if let Some(cond) = select.condition {
                // We have a condition for this selection. The resulting target is selected if we've been selected _and_
                // the condition is true.
                let cond = ResExpr::new(self, cond, context);
                ResExpr::And(Box::new(config_expr), Box::new(cond))
            } else {
                // No condition for this selection. The resulting target is selected if we've been selected.
                config_expr
            };

            target.borrow_mut().selected_by = Some(cond);
        }

        // Resolve each default value.
        for default in config.defaults.into_iter() {
            let value = ResExpr::new(self, default.value, context);
            let condition = match default.condition {
                Some(condition) => ResExpr::new(self, condition, context),
                None => ResExpr::Bool(true),
            };

            res_config.borrow_mut().defaults.push(ResConfigDefault {
                value,
                condition,
            });
        }

        // Resolve each dependency.
        for dep in config.depends_on.into_iter() {
            let dep = ResExpr::new(self, dep, context);
            res_config.borrow_mut().depends_on &= dep;
        }

        // Resolve each range.
        for range in config.ranges.into_iter() {
            let start = ResLitValue::resolve_new(range.start, self, context);
            let end = ResLitValue::resolve_new(range.end, self, context);
            let condition = if let Some(cond) = range.condition {
                ResExpr::new(self, cond, context)
            } else {
                ResExpr::Bool(true)
            };

            res_config.borrow_mut().ranges.push(ResConfigRange {
                start,
                end,
                condition,
            });
        }
    }
}
