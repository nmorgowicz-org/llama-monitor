// Compatibility executable retained throughout the 2.x line.
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
    eprintln!(
        "[compat] llama-monitor is deprecated; use local-llm-foundry. \
The legacy entrypoint remains supported through 2.x."
    );
    llama_monitor::runner::run()
}
