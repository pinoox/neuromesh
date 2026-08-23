use neuromesh_core::{parse_port, Config, Result};

pub fn execute(arg: Option<&str>) -> Result<()> {
    match arg {
        None | Some("get") | Some("-v") | Some("--show") => {
            let cfg = Config::load();
            println!(
                "Monitor port   : {}  (http://{}:{})",
                cfg.port, cfg.host, cfg.port
            );
            println!("Override with  : neuromesh port <n>  |  --port <n>  |  NEUROMESH_PORT");
            println!("Default        : {}", Config::DEFAULT_PORT);
        }
        Some("help") | Some("-h") | Some("--help") => print_usage(),
        Some(raw) => {
            let port = parse_port(raw)?;
            let cfg = Config::from_files().with_port(port);
            let path = cfg.save_local()?;
            println!("Monitor port set to {port}");
            println!("Saved          : {}", path.display());
            println!("Open           : http://{}:{port}", cfg.host);
            println!("Restart `neuromesh monitor` (and the VS Code / Cursor setting neuromesh.port) so clients follow.");
        }
    }
    Ok(())
}

fn print_usage() {
    println!(
        "\
Usage: neuromesh port [PORT]

Show or persist the galaxy monitor HTTP port (default {}).

  neuromesh port           print effective port (config + NEUROMESH_PORT)
  neuromesh port 9000      write <cwd>/.neuromesh/config.json
  neuromesh monitor --port 9000   one run only, does not save

VS Code / Cursor: Settings → neuromesh.port
",
        Config::DEFAULT_PORT
    );
}
