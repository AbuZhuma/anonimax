use crate::identity::Profile;
use crate::proxy::{Mode, Pool};
use crate::tor::Tor;
use std::sync::Mutex;
use std::time::Instant;
use wreq::{Client, Method};

pub struct Engine {
    state: Mutex<EngineState>,
}

struct EngineState {
    current: Profile,
    auto_rotate: bool,
    pool: Pool,
    tor: Tor,
}

pub struct ReqSpec {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

pub struct RespData {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub via: String,
    pub browser: &'static str,
    pub ms: u128,
}

pub struct Snapshot {
    pub browser: &'static str,
    pub auto: bool,
    pub tor_enabled: bool,
    pub tor_socks: String,
    pub tor_control: String,
    pub proxy_mode: &'static str,
    pub proxy_len: usize,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(EngineState {
                current: Profile::random(),
                auto_rotate: true,
                pool: Pool::new(),
                tor: Tor::new(),
            }),
        }
    }

    fn prepare(&self) -> anyhow::Result<(Client, Option<String>, &'static str, bool)> {
        let (profile, proxy, via_tor) = {
            let mut s = self.state.lock().unwrap();
            if s.auto_rotate {
                s.current = Profile::random();
            }
            if s.tor.enabled {
                (s.current, Some(s.tor.isolated_proxy()), true)
            } else {
                (s.current, s.pool.next(), false)
            }
        };
        let mut builder = Client::builder().emulation(profile.emulation());
        if let Some(pr) = &proxy {
            builder = builder.proxy(wreq::Proxy::all(pr.as_str())?);
        }
        let client = builder.build()?;
        Ok((client, proxy, profile.label, via_tor))
    }

    pub async fn execute(&self, spec: &ReqSpec) -> anyhow::Result<RespData> {
        let (client, proxy, label, via_tor) = self.prepare()?;
        let method = Method::from_bytes(spec.method.to_uppercase().as_bytes())
            .map_err(|_| anyhow::anyhow!("bad method: {}", spec.method))?;
        let mut rb = client.request(method, spec.url.as_str());
        for (k, v) in &spec.headers {
            rb = rb.header(k.as_str(), v.as_str());
        }
        if let Some(b) = &spec.body {
            rb = rb.body(b.clone());
        }
        let start = Instant::now();
        let resp = rb.send().await?;
        let ms = start.elapsed().as_millis();
        let status = resp.status().as_u16();
        let headers = resp
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let body = resp.text().await?;
        let via = if via_tor {
            "Tor".to_string()
        } else {
            proxy.unwrap_or_else(|| "<real IP>".to_string())
        };
        Ok(RespData { status, headers, body, via, browser: label, ms })
    }

    pub fn snapshot(&self) -> Snapshot {
        let s = self.state.lock().unwrap();
        Snapshot {
            browser: s.current.label,
            auto: s.auto_rotate,
            tor_enabled: s.tor.enabled,
            tor_socks: s.tor.socks.clone(),
            tor_control: s.tor.control.clone(),
            proxy_mode: s.pool.mode().label(),
            proxy_len: s.pool.len(),
        }
    }

    pub fn reset(&self) {
        let mut s = self.state.lock().unwrap();
        s.current = Profile::random();
        s.auto_rotate = true;
        s.pool.clear();
        s.tor.enabled = false;
    }

    pub fn set_current(&self, p: Profile) {
        self.state.lock().unwrap().current = p;
    }

    pub fn set_auto(&self, on: bool) {
        self.state.lock().unwrap().auto_rotate = on;
    }

    pub fn proxy_add(&self, url: String) {
        self.state.lock().unwrap().pool.add(url);
    }

    pub fn proxy_load(&self, contents: &str) -> usize {
        self.state.lock().unwrap().pool.load(contents)
    }

    pub fn proxy_list(&self) -> Vec<String> {
        self.state.lock().unwrap().pool.list().to_vec()
    }

    pub fn proxy_clear(&self) {
        self.state.lock().unwrap().pool.clear();
    }

    pub fn set_proxy_mode(&self, m: Mode) {
        self.state.lock().unwrap().pool.set_mode(m);
    }

    pub fn proxy_mode_label(&self) -> &'static str {
        self.state.lock().unwrap().pool.mode().label()
    }

    pub fn tor_set(&self, on: bool) {
        self.state.lock().unwrap().tor.enabled = on;
    }

    pub fn tor_socks(&self) -> String {
        self.state.lock().unwrap().tor.socks.clone()
    }

    pub fn tor_control(&self) -> String {
        self.state.lock().unwrap().tor.control.clone()
    }
}
