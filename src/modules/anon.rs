use crate::engine::{Engine, ReqSpec};
use crate::identity::{Profile, PROFILES};
use crate::module::{CmdInfo, Context, Module};
use crate::proxy::Mode;
use async_trait::async_trait;
use owo_colors::OwoColorize;
use std::sync::{Arc, Mutex};

const METHODS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];

pub struct Anon {
    engine: Arc<Engine>,
    headers: Mutex<Vec<(String, String)>>,
}

impl Anon {
    pub fn new(engine: Arc<Engine>) -> Self {
        Self { engine, headers: Mutex::new(Vec::new()) }
    }

    async fn send(&self, args: &[String]) -> anyhow::Result<()> {
        let rest = &args[1..];
        if rest.is_empty() {
            println!("{}", "usage: send [METHOD] <url> [body]".red());
            return Ok(());
        }
        let (method, url, body) = if METHODS.contains(&rest[0].to_uppercase().as_str()) {
            let Some(url) = rest.get(1) else {
                println!("{}", "usage: send <METHOD> <url> [body]".red());
                return Ok(());
            };
            let body = (rest.len() > 2).then(|| rest[2..].join(" "));
            (rest[0].to_uppercase(), normalize_url(url), body)
        } else {
            let body = (rest.len() > 1).then(|| rest[1..].join(" "));
            ("GET".to_string(), normalize_url(&rest[0]), body)
        };

        let spec = ReqSpec {
            method: method.clone(),
            url: url.clone(),
            headers: self.headers.lock().unwrap().clone(),
            body,
        };

        let r = self.engine.execute(&spec).await?;
        println!(
            "{} {} {} {} {}",
            "→".green().bold(),
            method.bold(),
            url.dimmed(),
            "as".dimmed(),
            r.browser.yellow()
        );
        let via = if r.via == "Tor" {
            r.via.magenta().to_string()
        } else if r.via == "<real IP>" {
            "<real IP>".red().to_string()
        } else {
            r.via.cyan().to_string()
        };
        println!("  {} {}", "via".dimmed(), via);

        let s = format!("{}", r.status);
        let colored = if (200..400).contains(&r.status) {
            s.green().to_string()
        } else {
            s.red().to_string()
        };
        println!(
            "{} {}  {}",
            "←".green().bold(),
            colored,
            format!("{} ms", r.ms).dimmed()
        );
        print_body(&r.body);
        Ok(())
    }

    async fn ip(&self) -> anyhow::Result<()> {
        println!("{}", "checking exit identity…".dimmed());
        let spec = ReqSpec {
            method: "GET".to_string(),
            url: "http://ip-api.com/json/".to_string(),
            headers: vec![],
            body: None,
        };
        let r = self.engine.execute(&spec).await?;
        if r.status != 200 {
            println!("{} HTTP {}", "leak check failed:".red(), r.status);
            return Ok(());
        }
        let v: serde_json::Value = serde_json::from_str(&r.body)?;
        println!("{}", "  as the server sees you:".bold().cyan());
        kv("exit IP", v["query"].as_str().unwrap_or("?"));
        kv(
            "location",
            &format!(
                "{}, {}",
                v["city"].as_str().unwrap_or("?"),
                v["country"].as_str().unwrap_or("?")
            ),
        );
        kv("ISP", v["isp"].as_str().unwrap_or("?"));
        kv("route", &r.via);
        kv("latency", &format!("{} ms", r.ms));
        Ok(())
    }

    async fn test(&self) -> anyhow::Result<()> {
        let spec = ReqSpec {
            method: "GET".to_string(),
            url: "https://tls.peet.ws/api/all".to_string(),
            headers: vec![],
            body: None,
        };
        let r = self.engine.execute(&spec).await?;
        println!("{} {}", "TLS fingerprint via".dimmed(), r.browser.yellow());
        print_body(&r.body);
        Ok(())
    }

    fn show_id(&self) {
        let s = self.engine.snapshot();
        let hcount = self.headers.lock().unwrap().len();
        println!("{}", "  current setup".bold().cyan());
        kv("browser/device", s.browser);
        kv("auto-rotate", if s.auto { "on" } else { "off" });
        kv(
            "route",
            if s.tor_enabled { "Tor (new IP per request)" } else { "direct / proxy pool" },
        );
        kv("proxy mode", s.proxy_mode);
        kv("proxy pool", &format!("{} loaded", s.proxy_len));
        kv("session headers", &format!("{hcount} set"));
    }

    async fn tor_cmd(&self, args: &[String]) -> anyhow::Result<()> {
        match args.get(1).map(|s| s.as_str()) {
            Some("on") => {
                let socks = self.engine.tor_socks();
                if !crate::tor::reachable(&socks).await {
                    println!("{} Tor SOCKS not reachable at {}", "error:".red(), socks);
                    println!(
                        "  {}",
                        "start Tor first:  sudo systemctl start tor".dimmed()
                    );
                    return Ok(());
                }
                self.engine.tor_set(true);
                println!("{}", "Tor: on — every request gets a fresh exit IP".green());
                self.ip().await?;
            }
            Some("off") => {
                self.engine.tor_set(false);
                println!("{}", "Tor: off".yellow());
            }
            Some("new") | Some("newnym") => {
                let control = self.engine.tor_control();
                match crate::tor::new_identity(&control).await {
                    Ok(()) => println!("{}", "Tor: requested new circuits (NEWNYM)".green()),
                    Err(e) => println!("{} {e}", "NEWNYM failed:".red()),
                }
            }
            Some("ip") | Some("check") => self.ip().await?,
            _ => {
                let s = self.engine.snapshot();
                println!("{}", "Tor".bold().underline());
                kv("status", if s.tor_enabled { "on" } else { "off" });
                kv("socks", &s.tor_socks);
                kv("control", &s.tor_control);
                println!("  {}", "commands: tor <on|off|new|ip>".dimmed());
            }
        }
        Ok(())
    }

    fn header_cmd(&self, args: &[String]) {
        match args.get(1).map(|s| s.as_str()) {
            Some("add") | Some("set") => match (args.get(2), args.get(3)) {
                (Some(k), Some(_)) => {
                    let v = args[3..].join(" ");
                    self.headers.lock().unwrap().push((k.clone(), v));
                    println!("{} {}", "header set:".green(), k);
                }
                _ => println!("{}", "usage: header add <Key> <Value>".red()),
            },
            Some("clear") => {
                self.headers.lock().unwrap().clear();
                println!("{}", "session headers cleared".yellow());
            }
            Some("list") | None => {
                let h = self.headers.lock().unwrap();
                if h.is_empty() {
                    println!("{}", "no session headers".dimmed());
                } else {
                    println!("{}", "session headers:".bold().underline());
                    for (k, v) in h.iter() {
                        println!("  {}: {}", k.cyan(), v);
                    }
                }
            }
            _ => println!("{}", "usage: header <add|list|clear>".red()),
        }
    }

    fn proxy_cmd(&self, args: &[String]) -> anyhow::Result<()> {
        match args.get(1).map(|s| s.as_str()) {
            Some("add") => match args.get(2) {
                Some(url) => {
                    self.engine.proxy_add(url.clone());
                    println!("{} {}", "added proxy:".green(), url);
                }
                None => println!("{}", "usage: proxy add socks5h://host:port".red()),
            },
            Some("load") => match args.get(2) {
                Some(path) => {
                    let contents = std::fs::read_to_string(path)?;
                    let n = self.engine.proxy_load(&contents);
                    println!("{} {} {}", "loaded".green(), n, "proxies".green());
                }
                None => println!("{}", "usage: proxy load <file>".red()),
            },
            Some("list") => {
                let list = self.engine.proxy_list();
                if list.is_empty() {
                    println!("{}", "proxy pool is empty".dimmed());
                } else {
                    println!(
                        "{} ({}):",
                        "proxy pool".bold().underline(),
                        self.engine.proxy_mode_label()
                    );
                    for (i, p) in list.iter().enumerate() {
                        println!("  {:>2}. {}", i + 1, p.cyan());
                    }
                }
            }
            Some("mode") => match args.get(2).map(|s| s.as_str()) {
                Some("off") => {
                    self.engine.set_proxy_mode(Mode::Off);
                    println!("{}", "proxy mode: off (real IP)".yellow());
                }
                Some("rotate") => {
                    self.engine.set_proxy_mode(Mode::Rotate);
                    println!("{}", "proxy mode: rotate".green());
                }
                Some("random") => {
                    self.engine.set_proxy_mode(Mode::Random);
                    println!("{}", "proxy mode: random".green());
                }
                _ => println!("{}", "usage: proxy mode <off|rotate|random>".red()),
            },
            Some("clear") => {
                self.engine.proxy_clear();
                println!("{}", "proxy pool cleared".yellow());
            }
            _ => println!(
                "{}",
                "usage: proxy <add <url> | load <file> | list | mode <off|rotate|random> | clear>"
                    .red()
            ),
        }
        Ok(())
    }
}

fn kv(k: &str, v: &str) {
    println!("    {:<16} {}", format!("{k}:").dimmed(), v);
}

fn print_body(body: &str) {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        if let Ok(pretty) = serde_json::to_string_pretty(&json) {
            println!("{pretty}");
            return;
        }
    }
    let preview: String = body.chars().take(800).collect();
    println!("{preview}");
    if body.len() > 800 {
        println!("{}", "  … (truncated)".dimmed());
    }
}

fn normalize_url(url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        format!("https://{url}")
    }
}

#[async_trait]
impl Module for Anon {
    fn name(&self) -> &'static str {
        "anon"
    }

    fn description(&self) -> &'static str {
        "Browser-grade TLS emulation + Tor/proxy, any HTTP method"
    }

    fn commands(&self) -> Vec<CmdInfo> {
        vec![
            CmdInfo { name: "send",    usage: "send [METHOD] <url> [body]", about: "Request with any method (GET/POST/PUT/PATCH/DELETE/HEAD/OPTIONS)" },
            CmdInfo { name: "header",  usage: "header <add|list|clear>",    about: "Manage headers added to every request" },
            CmdInfo { name: "tor",     usage: "tor <on|off|new|ip>",        about: "Route through Tor — fresh exit IP per request" },
            CmdInfo { name: "ip",      usage: "ip",                         about: "Leak check: exit IP / geo / ISP" },
            CmdInfo { name: "test",    usage: "test",                       about: "Inspect your TLS/JA3 fingerprint" },
            CmdInfo { name: "rotate",  usage: "rotate",                     about: "New random browser/device now" },
            CmdInfo { name: "browser", usage: "browser <name|list>",        about: "Pin a specific browser" },
            CmdInfo { name: "auto",    usage: "auto <on|off>",              about: "Re-roll the browser before every request" },
            CmdInfo { name: "proxy",   usage: "proxy <add|load|list|mode|clear>", about: "Manage the proxy pool" },
            CmdInfo { name: "id",      usage: "id",                         about: "Show the current setup" },
        ]
    }

    async fn run(&self, _ctx: &mut Context, args: &[String]) -> anyhow::Result<()> {
        match args.first().map(|s| s.as_str()).unwrap_or("") {
            "send" => self.send(args).await?,
            "test" => self.test().await?,
            "ip" | "leak" => self.ip().await?,
            "header" => self.header_cmd(args),
            "rotate" => {
                let p = Profile::random();
                self.engine.set_current(p);
                println!("{} {}", "rotated →".green(), p.label.yellow());
            }
            "browser" => match args.get(1).map(|s| s.as_str()) {
                Some("list") | None => {
                    println!("{}", "available browsers/devices:".bold().underline());
                    for p in PROFILES {
                        println!("  {}", p.label.yellow());
                    }
                }
                Some("random") => {
                    let p = Profile::random();
                    self.engine.set_current(p);
                    println!("{} {}", "set →".green(), p.label.yellow());
                }
                Some(q) => match Profile::find(q) {
                    Some(p) => {
                        self.engine.set_current(p);
                        self.engine.set_auto(false);
                        println!(
                            "{} {} {}",
                            "pinned →".green(),
                            p.label.yellow(),
                            "(auto-rotate off)".dimmed()
                        );
                    }
                    None => println!("{} {} — try `browser list`", "no match:".red(), q),
                },
            },
            "auto" => match args.get(1).map(|s| s.as_str()) {
                Some("on") => {
                    self.engine.set_auto(true);
                    println!("{}", "auto-rotate: on".green());
                }
                Some("off") => {
                    self.engine.set_auto(false);
                    println!("{}", "auto-rotate: off".yellow());
                }
                _ => println!("{}", "usage: auto <on|off>".red()),
            },
            "proxy" => self.proxy_cmd(args)?,
            "tor" => self.tor_cmd(args).await?,
            "id" | "identity" => self.show_id(),
            "" => println!("{}", "type `help` for anon commands".dimmed()),
            other => println!("{} {}", "unknown command:".red(), other),
        }
        Ok(())
    }
}
