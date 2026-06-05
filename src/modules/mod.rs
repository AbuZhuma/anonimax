mod anon;
mod gateway;
mod system;

use crate::engine::Engine;
use crate::module::Registry;
use std::sync::Arc;

pub fn registry() -> Registry {
    let engine = Arc::new(Engine::new());
    let mut reg = Registry::new();
    reg.add(Box::new(anon::Anon::new(engine.clone())));
    reg.add(Box::new(gateway::Gateway::new(engine.clone())));
    reg.add(Box::new(system::System::new()));
    reg
}
