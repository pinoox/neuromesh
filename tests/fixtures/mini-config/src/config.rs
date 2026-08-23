pub fn load_config(raw: &str) -> Config {
    Config {
        name: raw.trim().to_string(),
        debug: raw.contains("debug"),
    }
}

pub struct Config {
    pub name: String,
    pub debug: bool,
}
