//! Architecture-aware VRAM estimator for llama-server configurations.
//!
//! Handles:
//! - Standard full-attention (Llama, Mistral, Qwen, …)
//! - Sliding-window / alternating-attention (Gemma 3/4)
//! - MoE expert offloading (Mixtral, Qwen-MoE, DeepSeek, …)
//! - Multi-Token Prediction heads (DeepSeek-R1 style)
//! - Vision projector (mmproj) VRAM
//! - Pre-download quant comparison table
//! - Auto-size recommendation for a given use case

mod arch_heuristics;
mod estimate;
mod quant_table;

#[cfg(test)]
mod tests;

// ── Re-exports (match old flat module API) ─────────────────────────────────────

pub use arch_heuristics::*;
pub use estimate::*;
#[allow(unused_imports)]
pub use quant_table::*;
