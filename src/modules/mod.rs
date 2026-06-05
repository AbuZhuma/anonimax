mod anon;

use crate::module::Registry;

pub fn registry() -> Registry {
    let mut reg = Registry::new();
    reg.add(Box::new(anon::Anon::new()));
    reg
}
