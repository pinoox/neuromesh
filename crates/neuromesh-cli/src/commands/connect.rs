use neuromesh_core::{Config, Result};

pub fn execute() -> Result<()> {
    println!(
        "\n╔═══════════════════════════════════════════════════════════════════════════════════╗"
    );
    println!(
        "║              🔌 NEUROMESH V2 — 1-CLICK MCP CLIENT CONNECTION GUIDE                ║"
    );
    println!(
        "╚═══════════════════════════════════════════════════════════════════════════════════╝\n"
    );

    println!("1. Claude Desktop (claude_desktop_config.json):");
    println!("   ──────────────────────────────────────────────────────────────────────────");
    println!("   {{\n     \"mcpServers\": {{\n       \"neuromesh\": {{\n         \"command\": \"neuromesh\",\n         \"args\": [\"mcp\"]\n       }}\n     }}\n   }}\n");

    println!("2. Cursor IDE (.cursor/mcp.json or Settings > Features > MCP):");
    println!("   ──────────────────────────────────────────────────────────────────────────");
    println!("   {{\n     \"mcpServers\": {{\n       \"neuromesh\": {{\n         \"command\": \"neuromesh\",\n         \"args\": [\"mcp\"]\n       }}\n     }}\n   }}\n");

    println!("3. Cline / Roo Code / Roo-Cline (VS Code Extensions):");
    println!("   ──────────────────────────────────────────────────────────────────────────");
    println!("   Server Name : neuromesh\n   Command     : neuromesh\n   Arguments   : mcp\n");

    let cfg = Config::load();
    println!("4. Web UI Monitor Dashboard:");
    println!("   ──────────────────────────────────────────────────────────────────────────");
    println!("   Run: neuromesh monitor (or neuromesh ui)");
    println!("   URL: http://{}:{}", cfg.host, cfg.port);
    println!("   Port: neuromesh port 9000   or   neuromesh monitor --port 9000\n");

    Ok(())
}
