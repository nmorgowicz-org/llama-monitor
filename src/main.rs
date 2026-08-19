// Canonical Local LLM Foundry executable entrypoint.
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
    llama_monitor::runner::run()
}
