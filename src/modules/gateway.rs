use crate::engine::{Engine, ReqSpec};
use crate::module::{CmdInfo, Context, Module};
use async_trait::async_trait;
use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use owo_colors::OwoColorize;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

pub struct Gateway {
    engine: Arc<Engine>,
    running: Mutex<Option<Running>>,
}

struct Running {
    port: u16,
    shutdown: oneshot::Sender<()>,
}

#[derive(Deserialize)]
struct ReqIn {
    method: Option<String>,
    url: String,
    headers: Option<HashMap<String, String>>,
    body: Option<String>,
}

async fn root() -> &'static str {
    "anonimax gateway\n\nPOST /request\n  {\"method\":\"GET\",\"url\":\"https://example.com\",\"headers\":{\"X-Foo\":\"bar\"},\"body\":null}\n\nresponse\n  {\"status\":200,\"headers\":{...},\"body\":\"...\",\"via\":\"Tor\",\"browser\":\"Chrome 137 / Windows\",\"ms\":842}\n"
}

async fn handle(State(engine): State<Arc<Engine>>, Json(input): Json<ReqIn>) -> Json<serde_json::Value> {
    let spec = ReqSpec {
        method: input.method.unwrap_or_else(|| "GET".to_string()),
        url: input.url,
        headers: input
            .headers
            .map(|m| m.into_iter().collect())
            .unwrap_or_default(),
        body: input.body,
    };
    match engine.execute(&spec).await {
        Ok(r) => {
            let headers: serde_json::Map<String, serde_json::Value> = r
                .headers
                .into_iter()
                .map(|(k, v)| (k, serde_json::Value::String(v)))
                .collect();
            Json(serde_json::json!({
                "status": r.status,
                "headers": headers,
                "body": r.body,
                "via": r.via,
                "browser": r.browser,
                "ms": r.ms,
            }))
        }
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

impl Gateway {
    pub fn new(engine: Arc<Engine>) -> Self {
        Self { engine, running: Mutex::new(None) }
    }

    async fn start(&self, port: u16) -> anyhow::Result<()> {
        if self.running.lock().unwrap().is_some() {
            println!("{}", "gateway already running — `gateway stop` first".yellow());
            return Ok(());
        }
        let listener = TcpListener::bind(("127.0.0.1", port)).await?;
        let (tx, rx) = oneshot::channel::<()>();
        let app = Router::new()
            .route("/", get(root))
            .route("/request", post(handle))
            .with_state(self.engine.clone());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = rx.await;
                })
                .await;
        });
        *self.running.lock().unwrap() = Some(Running { port, shutdown: tx });

        println!("{} http://127.0.0.1:{}", "gateway listening on".green().bold(), port);
        println!("  {}", "point any code at it, e.g.:".dimmed());
        println!(
            "  {}",
            format!(
                "curl -s 127.0.0.1:{port}/request -d '{{\"method\":\"GET\",\"url\":\"https://api.ipify.org\"}}'"
            )
            .cyan()
        );
        Ok(())
    }

    fn stop(&self) {
        match self.running.lock().unwrap().take() {
            Some(r) => {
                let _ = r.shutdown.send(());
                println!("{} (was on :{})", "gateway stopped".yellow(), r.port);
            }
            None => println!("{}", "gateway is not running".dimmed()),
        }
    }

    fn status(&self) {
        match self.running.lock().unwrap().as_ref() {
            Some(r) => println!("{} http://127.0.0.1:{}", "gateway: running on".green(), r.port),
            None => println!("{}", "gateway: stopped".dimmed()),
        }
    }
}

#[async_trait]
impl Module for Gateway {
    fn name(&self) -> &'static str {
        "gateway"
    }

    fn description(&self) -> &'static str {
        "Local HTTP server — route your own code's requests through anonimax"
    }

    fn commands(&self) -> Vec<CmdInfo> {
        vec![
            CmdInfo { name: "start",  usage: "start [port]", about: "Start local gateway (default port 8888)" },
            CmdInfo { name: "stop",   usage: "stop",         about: "Stop the gateway" },
            CmdInfo { name: "status", usage: "status",       about: "Show gateway state" },
        ]
    }

    async fn reset(&self) {
        if let Some(r) = self.running.lock().unwrap().take() {
            let _ = r.shutdown.send(());
        }
    }

    async fn run(&self, _ctx: &mut Context, args: &[String]) -> anyhow::Result<()> {
        match args.first().map(|s| s.as_str()).unwrap_or("") {
            "start" => {
                let port = args
                    .get(1)
                    .and_then(|p| p.parse::<u16>().ok())
                    .unwrap_or(8888);
                if let Err(e) = self.start(port).await {
                    println!("{} {e}", "failed to start:".red());
                }
            }
            "stop" => self.stop(),
            "status" => self.status(),
            "" => println!("{}", "usage: start [port] | stop | status".dimmed()),
            other => println!("{} {}", "unknown command:".red(), other),
        }
        Ok(())
    }
}
