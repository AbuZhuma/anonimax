use rand::seq::SliceRandom;

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Off,
    Rotate,
    Random,
}

impl Mode {
    pub fn label(&self) -> &'static str {
        match self {
            Mode::Off => "off",
            Mode::Rotate => "rotate (round-robin)",
            Mode::Random => "random",
        }
    }
}

pub struct Pool {
    proxies: Vec<String>,
    mode: Mode,
    cursor: usize,
}

impl Pool {
    pub fn new() -> Self {
        Self { proxies: Vec::new(), mode: Mode::Off, cursor: 0 }
    }

    pub fn add(&mut self, url: String) {
        self.proxies.push(url);
        if self.mode == Mode::Off {
            self.mode = Mode::Rotate;
        }
    }

    pub fn load(&mut self, contents: &str) -> usize {
        let mut n = 0;
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            self.add(line.to_string());
            n += 1;
        }
        n
    }

    pub fn clear(&mut self) {
        self.proxies.clear();
        self.cursor = 0;
        self.mode = Mode::Off;
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn len(&self) -> usize {
        self.proxies.len()
    }

    pub fn list(&self) -> &[String] {
        &self.proxies
    }

    pub fn next(&mut self) -> Option<String> {
        if self.proxies.is_empty() {
            return None;
        }
        match self.mode {
            Mode::Off => None,
            Mode::Rotate => {
                let p = self.proxies[self.cursor % self.proxies.len()].clone();
                self.cursor = self.cursor.wrapping_add(1);
                Some(p)
            }
            Mode::Random => {
                let mut rng = rand::thread_rng();
                self.proxies.choose(&mut rng).cloned()
            }
        }
    }
}
