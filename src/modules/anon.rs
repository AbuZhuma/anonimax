use crate::identity::{Profile, PROFILES};
use crate::module::{CmdInfo, Context, Module};
use crate::proxy::{Mode, Pool};
use async_trait::async_trait;
use owo_colors::OwoColorize;
use std::sync::Mutex;
use std::time::Instant;
use wreq::Client;

pub struct Anon {
    state: Mutex<State>,
}

struct State {
    current: Profile,
    auto_rotate: bool,
    pool: Pool,
}

impl Anon {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                current: Profile::random(),
                auto_rotate: true,
                pool: Pool::new(),
            }),
        }
    }

    fn prepare(&self) -> anyhow::Result<(Client, Option<String>, &'static str)> {
        let (profile, proxy) = {
            let mut s = self.state.lock().unwrap();
            if s.auto_rotate {
                s.current = Profile::random();
            }
            let proxy = s.pool.next();
            (s.current, proxy)
        };
        let client = Client::builder().emulation(profile.emulation()).build()?;
        Ok((client, proxy, profile.label))
    }

    async fn get_text(&self, url: &str) -> anyhow::Result<(u16, u128, String, String)> {
        let (client, proxy, label) = self.prepare()?;
        let mut rb = client.get(url);
        if let Some(p) = &proxy {
            rb = rb.proxy(p.as_str());
        }
        let start = Instant::now();
        let resp = rb.send().await?;
        let status = resp.status().as_u16();
        let body = resp.text().await?;
        let ms = start.elapsed().as_millis();
        let via = proxy.unwrap_or_else(|| "<real IP>".to_string());
        let _ = label;
        Ok((status, ms, body, via))
    }

    async fn send(&self, url: &str) -> anyhow::Result<()> {
        let (client, proxy, label) = self.prepare()?;
        println!(
            "{} {} {}",
            "→".green().bold(),
            "as".dimmed(),
            label.yellow()
        );
        match &proxy {
            Some(p) => println!("  {} {}", "via".dimmed(), p.cyan()),
            None => println!("  {}", "no proxy — using your REAL IP".red()),
        }

        let mut rb = client.get(url);
        if let Some(p) = &proxy {
            rb = rb.proxy(p.as_str());
        }
        let start = Instant::now();
        let resp = rb.send().await?;
        let ms = start.elapsed().as_millis();
        let status = resp.status();
        let s = format!("{status}");
        let colored = if status.is_success() {
            s.green().to_string()
        } else if status.is_client_error() || status.is_server_error() {
            s.red().to_string()
        } else {
            s.yellow().to_string()
        };
        let final_url = resp.url().to_string();
        let body = resp.text().await?;
        println!(
            "{} {}  {}  {}",
            "←".green().bold(),
            colored,
            format!("{ms} ms").dimmed(),
            final_url.dimmed()
        );
        print_body(&body);
        Ok(())
    }

    async fn ip(&self) -> anyhow::Result<()> {
        println!("{}", "checking exit identity…".dimmed());
        let (status, ms, body, via) = self.get_text("http://ip-api.com/json/").await?;
        if status != 200 {
            println!("{} HTTP {status}", "leak check failed:".red());
            return Ok(());
        }
        let v: serde_json::Value = serde_json::from_str(&body)?;
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
        kv("route", &via);
        kv("latency", &format!("{ms} ms"));
        Ok(())
    }

    fn show_id(&self) {
        let s = self.state.lock().unwrap();
        println!("{}", "  current setup".bold().cyan());
        kv("browser/device", s.current.label);
        kv("auto-rotate", if s.auto_rotate { "on" } else { "off" });
        kv("proxy mode", s.pool.mode().label());
        kv("proxy pool", &format!("{} loaded", s.pool.len()));
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
    let preview: String = body.chars().take(700).collect();
    println!("{preview}");
    if body.len() > 700 {
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
        "Browser-grade TLS emulation + rotating proxy pool (fingerprint & IP)"
    }

    fn commands(&self) -> Vec<CmdInfo> {
        vec![
            CmdInfo { name: "send",     usage: "send <url>",            about: "GET a URL with a full browser emulation" },
            CmdInfo { name: "ip",       usage: "ip",                    about: "Leak check: show exit IP / geo / ISP a server sees" },
            CmdInfo { name: "test",     usage: "test",                  about: "Inspect your TLS/JA3 fingerprint (tls.peet.ws)" },
            CmdInfo { name: "rotate",   usage: "rotate",                about: "Switch to a new random browser/device now" },
            CmdInfo { name: "browser",  usage: "browser <name|list>",   about: "Pin a specific browser (e.g. `browser firefox`)" },
            CmdInfo { name: "auto",     usage: "auto <on|off>",         about: "Re-roll the browser before every request" },
            CmdInfo { name: "proxy",    usage: "proxy <add|load|list|mode|clear>", about: "Manage the proxy pool (changes your IP)" },
            CmdInfo { name: "id",       usage: "id",                    about: "Show the current setup" },
        ]
    }

    async fn run(&self, _ctx: &mut Context, args: &[String]) -> anyhow::Result<()> {
        let cmd = args.first().map(|s| s.as_str()).unwrap_or("");
        match cmd {
            "send" => match args.get(1) {
                Some(url) => self.send(&normalize_url(url)).await?,
                None => println!("{}", "usage: send <url>".red()),
            },
            "test" => self.send("https://tls.peet.ws/api/all").await?,
            "ip" | "leak" => self.ip().await?,
            "rotate" => {
                let p = Profile::random();
                self.state.lock().unwrap().current = p;
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
                    self.state.lock().unwrap().current = p;
                    println!("{} {}", "set →".green(), p.label.yellow());
                }
                Some(q) => match Profile::find(q) {
                    Some(p) => {
                        self.state.lock().unwrap().current = p;
                        self.state.lock().unwrap().auto_rotate = false;
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
                    self.state.lock().unwrap().auto_rotate = true;
                    println!("{}", "auto-rotate: on".green());
                }
                Some("off") => {
                    self.state.lock().unwrap().auto_rotate = false;
                    println!("{}", "auto-rotate: off".yellow());
                }
                _ => println!("{}", "usage: auto <on|off>".red()),
            },
            "proxy" => self.proxy_cmd(args).await?,
            "id" | "identity" => self.show_id(),
            "" => println!("{}", "type `help` for anon commands".dimmed()),
            other => println!("{} {}", "unknown command:".red(), other),
        }
        Ok(())
    }
}

impl Anon {
    async fn proxy_cmd(&self, args: &[String]) -> anyhow::Result<()> {
        match args.get(1).map(|s| s.as_str()) {
            Some("add") => match args.get(2) {
                Some(url) => {
                    self.state.lock().unwrap().pool.add(url.clone());
                    println!("{} {}", "added proxy:".green(), url);
                }
                None => println!(
                    "{}",
                    "usage: proxy add socks5h://host:port  (or http://user:pass@host:port)".red()
                ),
            },
            Some("load") => match args.get(2) {
                Some(path) => {
                    let contents = std::fs::read_to_string(path)?;
                    let n = self.state.lock().unwrap().pool.load(&contents);
                    println!("{} {} {}", "loaded".green(), n, "proxies".green());
                }
                None => println!("{}", "usage: proxy load <file>".red()),
            },
            Some("list") => {
                let s = self.state.lock().unwrap();
                if s.pool.len() == 0 {
                    println!("{}", "proxy pool is empty".dimmed());
                } else {
                    println!(
                        "{} ({}):",
                        "proxy pool".bold().underline(),
                        s.pool.mode().label()
                    );
                    for (i, p) in s.pool.list().iter().enumerate() {
                        println!("  {:>2}. {}", i + 1, p.cyan());
                    }
                }
            }
            Some("mode") => match args.get(2).map(|s| s.as_str()) {
                Some("off") => {
                    self.state.lock().unwrap().pool.set_mode(Mode::Off);
                    println!("{}", "proxy mode: off (real IP)".yellow());
                }
                Some("rotate") => {
                    self.state.lock().unwrap().pool.set_mode(Mode::Rotate);
                    println!("{}", "proxy mode: rotate".green());
                }
                Some("random") => {
                    self.state.lock().unwrap().pool.set_mode(Mode::Random);
                    println!("{}", "proxy mode: random".green());
                }
                _ => println!("{}", "usage: proxy mode <off|rotate|random>".red()),
            },
            Some("clear") => {
                self.state.lock().unwrap().pool.clear();
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
