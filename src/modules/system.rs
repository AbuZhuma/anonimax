use crate::module::{CmdInfo, Context, Module};
use async_trait::async_trait;
use owo_colors::OwoColorize;
use tokio::process::Command;

const ENABLE_SCRIPT: &str = r#"
set -e
TORRC=/etc/tor/torrc
if ! grep -q '# >>> anonimax >>>' "$TORRC"; then
  cat >> "$TORRC" <<'EOF'

# >>> anonimax >>>
VirtualAddrNetworkIPv4 10.192.0.0/10
AutomapHostsOnResolve 1
TransPort 9040
DNSPort 5353
# <<< anonimax <<<
EOF
fi

systemctl restart tor
sleep 2

mkdir -p /var/lib/anonimax
[ -f /var/lib/anonimax/iptables.backup ] || iptables-save > /var/lib/anonimax/iptables.backup
[ -f /var/lib/anonimax/ip6tables.backup ] || ip6tables-save > /var/lib/anonimax/ip6tables.backup

TOR_UID=$(id -u tor)
NON_TOR="127.0.0.0/8 192.168.0.0/16 10.0.0.0/8 172.16.0.0/12 169.254.0.0/16"

ip6tables -F OUTPUT
ip6tables -P OUTPUT DROP

iptables -F OUTPUT
iptables -t nat -F OUTPUT

iptables -t nat -A OUTPUT -m owner --uid-owner $TOR_UID -j RETURN
iptables -t nat -A OUTPUT -p udp --dport 53 -j REDIRECT --to-ports 5353
iptables -t nat -A OUTPUT -p tcp -d 10.192.0.0/10 -j REDIRECT --to-ports 9040
iptables -t nat -A OUTPUT -p udp -d 10.192.0.0/10 -j REDIRECT --to-ports 9040
for net in $NON_TOR; do iptables -t nat -A OUTPUT -d $net -j RETURN; done
iptables -t nat -A OUTPUT -p tcp --syn -j REDIRECT --to-ports 9040

iptables -A OUTPUT -m state --state ESTABLISHED,RELATED -j ACCEPT
for net in $NON_TOR; do iptables -A OUTPUT -d $net -j ACCEPT; done
iptables -A OUTPUT -m owner --uid-owner $TOR_UID -j ACCEPT
iptables -A OUTPUT -j REJECT
echo OK
"#;

const DISABLE_SCRIPT: &str = r#"
set -e
if [ -f /var/lib/anonimax/iptables.backup ]; then
  iptables-restore < /var/lib/anonimax/iptables.backup
  rm -f /var/lib/anonimax/iptables.backup
else
  iptables -F OUTPUT
  iptables -t nat -F OUTPUT
  iptables -P OUTPUT ACCEPT
fi

if [ -f /var/lib/anonimax/ip6tables.backup ]; then
  ip6tables-restore < /var/lib/anonimax/ip6tables.backup
  rm -f /var/lib/anonimax/ip6tables.backup
else
  ip6tables -F OUTPUT
  ip6tables -P OUTPUT ACCEPT
fi
echo OK
"#;

pub struct System;

impl System {
    pub fn new() -> Self {
        System
    }

    async fn run_root(&self, script: &str) -> anyhow::Result<bool> {
        let status = Command::new("sudo")
            .arg("bash")
            .arg("-c")
            .arg(script)
            .status()
            .await?;
        Ok(status.success())
    }

    async fn status(&self) -> anyhow::Result<()> {
        println!("{}", "checking whole-device route…".dimmed());
        let client = wreq::Client::builder().build()?;
        let body = client
            .get("https://check.torproject.org/api/ip")
            .send()
            .await?
            .text()
            .await?;
        let v: serde_json::Value = serde_json::from_str(&body)?;
        let is_tor = v["IsTor"].as_bool().unwrap_or(false);
        let ip = v["IP"].as_str().unwrap_or("?");
        if is_tor {
            println!("{} {}", "all traffic via Tor — exit IP:".green(), ip.yellow());
        } else {
            println!(
                "{} {}",
                "NOT routed through Tor — real IP:".red(),
                ip.yellow()
            );
        }
        Ok(())
    }
}

#[async_trait]
impl Module for System {
    fn name(&self) -> &'static str {
        "system"
    }

    fn description(&self) -> &'static str {
        "Route the WHOLE device through Tor via firewall (needs root)"
    }

    fn commands(&self) -> Vec<CmdInfo> {
        vec![
            CmdInfo { name: "on",     usage: "on",     about: "Force all device TCP+DNS through Tor (sudo)" },
            CmdInfo { name: "off",    usage: "off",    about: "Restore normal networking (sudo)" },
            CmdInfo { name: "status", usage: "status", about: "Check whether the whole device exits via Tor" },
        ]
    }

    async fn run(&self, _ctx: &mut Context, args: &[String]) -> anyhow::Result<()> {
        match args.first().map(|s| s.as_str()).unwrap_or("") {
            "on" => {
                println!(
                    "{}",
                    "enabling whole-device Tor routing (sudo password may be asked)…".dimmed()
                );
                if self.run_root(ENABLE_SCRIPT).await? {
                    println!("{}", "system Tor routing: ON".green().bold());
                    println!(
                        "  {}",
                        "every app, browser and script now exits via Tor".dimmed()
                    );
                    self.status().await?;
                } else {
                    println!("{}", "failed to enable (sudo denied or error)".red());
                }
            }
            "off" => {
                println!("{}", "restoring normal networking (sudo)…".dimmed());
                if self.run_root(DISABLE_SCRIPT).await? {
                    println!("{}", "system Tor routing: OFF".yellow().bold());
                } else {
                    println!("{}", "failed to restore (sudo denied or error)".red());
                }
            }
            "status" | "check" => self.status().await?,
            "" => println!("{}", "usage: on | off | status".dimmed()),
            other => println!("{} {}", "unknown command:".red(), other),
        }
        Ok(())
    }
}
