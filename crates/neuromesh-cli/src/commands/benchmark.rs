use super::evaluate;

/// Workspace measurement. The old synthetic 99.6% suite was not this engine.
pub fn execute() -> neuromesh_core::Result<()> {
    println!(
        "\nNeuroMesh benchmark runs the same honest workspace evaluation as `neuromesh eval`."
    );
    println!("It indexes the current directory and scores gold (or builtin) tasks under real fill caps.\n");
    evaluate::execute()
}
