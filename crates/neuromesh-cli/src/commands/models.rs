use neuromesh_core::{Config, Result};
use neuromesh_local_ai::LocalModelDescriptor;
use neuromesh_provider::ProviderFactory;

pub fn execute() -> Result<()> {
    let config = Config::default();
    let provider = ProviderFactory::create(&config.provider);
    let models = provider.model_info();

    println!("\n🤖 Configured Models");
    println!("===============================================");

    println!("{:<30} {:<25} {:<18} {:<10}", "Model ID", "Provider / Engine", "Context Window", "Streaming");
    println!("{:-<85}", "");

    for m in models {
        println!(
            "{:<30} {:<25} {:<18} {:<10}",
            m.id,
            provider.name(),
            format!("{}k", m.context_window / 1000),
            if m.supports_streaming { "Yes" } else { "No" }
        );
    }

    let local_models = vec![
        LocalModelDescriptor::qwen_0_6b(),
        LocalModelDescriptor::qwen_1_5b(),
        LocalModelDescriptor::llama_3b(),
    ];

    for lm in local_models {
        println!(
            "{:<30} {:<25} {:<18} {:<10}",
            lm.name.chars().take(28).collect::<String>(),
            format!("Local GGUF ({})", lm.parameter_size),
            "8k",
            "Yes"
        );
    }
    println!();

    Ok(())
}
