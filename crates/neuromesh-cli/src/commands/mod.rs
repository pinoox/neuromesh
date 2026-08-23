pub mod benchmark;
pub mod connect;
pub mod doctor;
pub mod evaluate;
pub mod graph;
pub mod index;
pub mod init;
pub mod memory;
pub mod models;
pub mod monitor;
pub mod optimize;
pub mod port;
pub mod start;
pub mod status;

use neuromesh_core::{parse_port, Result};

/// `--port 9000`, `-p 9000`, or `--port=9000` anywhere in argv.
pub fn port_from_args(args: &[String]) -> Result<Option<u16>> {
    for (i, a) in args.iter().enumerate() {
        if a == "--port" || a == "-p" {
            let raw = args.get(i + 1).ok_or_else(|| {
                neuromesh_core::NeuroMeshError::Config("--port needs a number".into())
            })?;
            return Ok(Some(parse_port(raw)?));
        }
        if let Some(raw) = a.strip_prefix("--port=") {
            return Ok(Some(parse_port(raw)?));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::port_from_args;

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn parses_port_flags() {
        assert_eq!(
            port_from_args(&args(&["monitor", "--port", "9000"])).unwrap(),
            Some(9000)
        );
        assert_eq!(
            port_from_args(&args(&["monitor", "-p", "9001"])).unwrap(),
            Some(9001)
        );
        assert_eq!(
            port_from_args(&args(&["monitor", "--port=9002"])).unwrap(),
            Some(9002)
        );
        assert_eq!(port_from_args(&args(&["monitor"])).unwrap(), None);
        assert!(port_from_args(&args(&["monitor", "--port"])).is_err());
        assert!(port_from_args(&args(&["monitor", "--port", "0"])).is_err());
    }
}
