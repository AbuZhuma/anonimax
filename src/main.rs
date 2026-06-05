mod engine;
mod identity;
mod module;
mod modules;
mod proxy;
mod shell;
mod tor;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let registry = modules::registry();
    shell::run(registry).await
}
