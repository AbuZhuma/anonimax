use crate::module::{Context, Registry};
use owo_colors::OwoColorize;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

const BANNER: &str = r#"
                          _
   __ _ _ __   ___  _ __ (_)_ __ ___   __ ___  __
  / _` | '_ \ / _ \| '_ \| | '_ ` _ \ / _` \ \/ /
 | (_| | | | | (_) | | | | | | | | | | (_| |>  <
  \__,_|_| |_|\___/|_| |_|_|_| |_| |_|\__,_/_/\_\
"#;

pub async fn run(registry: Registry) -> anyhow::Result<()> {
    let mut ctx = Context::default();
    let mut rl = DefaultEditor::new()?;

    println!("{}", BANNER.bright_magenta());
    println!(
        "  {} {}",
        "modular anti-detect panel".bright_white().bold(),
        format!("v{}", env!("CARGO_PKG_VERSION")).dimmed()
    );
    println!(
        "  {}\n",
        "type `help` to start, `modules` to list, `exit` to quit".dimmed()
    );

    loop {
        let prompt = build_prompt(&ctx);
        match rl.readline(&prompt) {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(line);
                if dispatch(&registry, &mut ctx, line).await {
                    break;
                }
            }
            Err(ReadlineError::Interrupted) => {
                if ctx.active.take().is_none() {
                    println!("{}", "(ctrl-c) type `exit` to quit".dimmed());
                }
            }
            Err(ReadlineError::Eof) => break,
            Err(e) => {
                println!("{} {e}", "input error:".red());
                break;
            }
        }
    }
    println!("{}", "bye.".dimmed());
    Ok(())
}

fn build_prompt(ctx: &Context) -> String {
    match &ctx.active {
        Some(m) => format!(
            "{}{}{} ",
            "anonimax(".bright_magenta(),
            m.yellow(),
            ")>".bright_magenta()
        ),
        None => format!("{} ", "anonimax>".bright_magenta()),
    }
}

async fn dispatch(registry: &Registry, ctx: &mut Context, line: &str) -> bool {
    let args: Vec<String> = line.split_whitespace().map(String::from).collect();
    let cmd = args[0].as_str();

    match cmd {
        "exit" | "quit" => return true,
        "modules" | "ls" => {
            list_modules(registry);
            return false;
        }
        "use" => {
            match args.get(1) {
                Some(name) if registry.get(name).is_some() => {
                    ctx.active = Some(name.clone());
                    println!("{} {}", "switched to module:".green(), name.yellow());
                }
                Some(name) => println!("{} {}", "no such module:".red(), name),
                None => println!("{}", "usage: use <module>".red()),
            }
            return false;
        }
        "back" => {
            if ctx.active.take().is_none() {
                println!("{}", "not inside a module".dimmed());
            }
            return false;
        }
        "help" | "?" => {
            help(registry, ctx);
            return false;
        }
        _ => {}
    }

    match ctx.active.clone() {
        Some(name) => {
            let module = registry.get(&name).expect("active module exists");
            if let Err(e) = module.run(ctx, &args).await {
                println!("{} {e}", "error:".red().bold());
            }
        }
        None => {
            println!(
                "{} `{}` — type `help`, or `use <module>` first",
                "unknown command:".red(),
                cmd
            );
        }
    }
    false
}

fn list_modules(registry: &Registry) {
    println!("{}", "available modules:".bold().underline());
    for m in registry.all() {
        println!(
            "  {:<10} {}",
            m.name().yellow().bold(),
            m.description().dimmed()
        );
    }
}

fn help(registry: &Registry, ctx: &Context) {
    println!("{}", "global commands:".bold().underline());
    let globals = [
        ("modules", "list available modules"),
        ("use <module>", "enter a module"),
        ("back", "leave the current module"),
        ("help", "show this help"),
        ("exit", "quit anonimax"),
    ];
    for (c, d) in globals {
        println!("  {:<16} {}", c.cyan(), d.dimmed());
    }

    if let Some(name) = &ctx.active {
        if let Some(m) = registry.get(name) {
            println!("\n{} {}", "module:".bold().underline(), name.yellow().bold());
            for c in m.commands() {
                println!("  {:<22} {}", c.usage.cyan(), c.about.dimmed());
            }
        }
    } else {
        println!("\n{}", "enter a module with `use <module>` to see its commands".dimmed());
    }
}
