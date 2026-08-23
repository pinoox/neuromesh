use crate::config::{load_config, Config};

pub fn boot(raw: &str) -> Config {
    let cfg = load_config(raw);
    apply_config(&cfg);
    cfg
}

fn apply_config(cfg: &Config) {
    let _ = cfg.debug;
}
