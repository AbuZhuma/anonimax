use async_trait::async_trait;
use std::collections::BTreeMap;

#[derive(Default)]
pub struct Context {
    pub active: Option<String>,
}

#[allow(dead_code)]
pub struct CmdInfo {
    pub name: &'static str,
    pub usage: &'static str,
    pub about: &'static str,
}

#[async_trait]
pub trait Module: Send + Sync {
    fn name(&self) -> &'static str;

    fn description(&self) -> &'static str;

    fn commands(&self) -> Vec<CmdInfo>;

    async fn run(&self, ctx: &mut Context, args: &[String]) -> anyhow::Result<()>;
}

pub struct Registry {
    modules: BTreeMap<&'static str, Box<dyn Module>>,
}

impl Registry {
    pub fn new() -> Self {
        Self { modules: BTreeMap::new() }
    }

    pub fn add(&mut self, module: Box<dyn Module>) {
        self.modules.insert(module.name(), module);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Module> {
        self.modules.get(name).map(|m| m.as_ref())
    }

    pub fn all(&self) -> impl Iterator<Item = &dyn Module> {
        self.modules.values().map(|m| m.as_ref())
    }
}
