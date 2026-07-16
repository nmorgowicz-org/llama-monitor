use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;

use crate::inference::InferenceBackend;
use crate::inference::capabilities::CapabilitySet;
use crate::inference::llama_cpp::LlamaCppAdapter;
use crate::inference::metrics::InferenceMetricsSnapshot;
use crate::inference::rapid_mlx::RapidMlxAdapter;
use crate::inference::supervisor::SupervisedLaunch;

#[derive(Debug, Clone, Copy, Default, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationArtifactKind {
    Gguf,
    MlxDirectory,
    AuthoritativeSafetensors,
    RapidMlxHfRepository,
    RapidMlxAlias,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct BackendRecommendationInput {
    #[serde(default)]
    pub artifact_kind: RecommendationArtifactKind,
    #[serde(default)]
    pub rapid_mlx_local_available: bool,
    #[serde(default)]
    pub rapid_mlx_runtime_compatible: bool,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct BackendRecommendation {
    pub recommended_backend: Option<InferenceBackend>,
    pub state: &'static str,
    pub reason: &'static str,
}

/// Return an explainable engine recommendation without inspecting filenames or
/// silently substituting one backend for another.
pub fn recommend_backend(input: &BackendRecommendationInput) -> BackendRecommendation {
    match input.artifact_kind {
        RecommendationArtifactKind::Gguf => BackendRecommendation {
            recommended_backend: Some(InferenceBackend::LlamaCpp),
            state: "ready",
            reason: "GGUF runs natively with llama.cpp.",
        },
        RecommendationArtifactKind::MlxDirectory
        | RecommendationArtifactKind::AuthoritativeSafetensors
        | RecommendationArtifactKind::RapidMlxHfRepository
        | RecommendationArtifactKind::RapidMlxAlias
            if !input.rapid_mlx_local_available =>
        {
            BackendRecommendation {
                recommended_backend: None,
                state: "platform_unavailable",
                reason: "Local Rapid-MLX requires Apple Silicon macOS; remote attachment remains available.",
            }
        }
        RecommendationArtifactKind::MlxDirectory
        | RecommendationArtifactKind::AuthoritativeSafetensors
        | RecommendationArtifactKind::RapidMlxHfRepository
        | RecommendationArtifactKind::RapidMlxAlias
            if !input.rapid_mlx_runtime_compatible =>
        {
            BackendRecommendation {
                recommended_backend: None,
                state: "runtime_required",
                reason: "This model is Rapid-MLX compatible, but a compatible local runtime is required.",
            }
        }
        RecommendationArtifactKind::MlxDirectory
        | RecommendationArtifactKind::AuthoritativeSafetensors
        | RecommendationArtifactKind::RapidMlxHfRepository
        | RecommendationArtifactKind::RapidMlxAlias => BackendRecommendation {
            recommended_backend: Some(InferenceBackend::RapidMlx),
            state: "ready",
            reason: "This source is native to the verified Rapid-MLX resolution path.",
        },
        RecommendationArtifactKind::Unknown => BackendRecommendation {
            recommended_backend: None,
            state: "manual_selection",
            reason: "Choose an engine after selecting a typed model source.",
        },
    }
}

/// The BackendAdapter is an enum to ensure exhaustive and zero-overhead dispatch
/// to the specific implementation for each inference engine.
#[allow(dead_code)]
#[derive(Clone)]
pub enum BackendAdapter {
    LlamaCpp(Arc<LlamaCppAdapter>),
    RapidMlx(Arc<RapidMlxAdapter>),
}

#[allow(dead_code)]
impl BackendAdapter {
    /// Validate platform, runtime, model source, and flag conflicts before launch.
    pub async fn validate(&self) -> Result<()> {
        match self {
            Self::LlamaCpp(adapter) => adapter.validate().await,
            Self::RapidMlx(adapter) => adapter.validate().await,
        }
    }

    /// Build the launch bundle. Called after validate() succeeds.
    pub async fn build_launch(&self) -> Result<SupervisedLaunch> {
        match self {
            Self::LlamaCpp(adapter) => adapter.build_launch().await,
            Self::RapidMlx(adapter) => adapter.build_launch().await,
        }
    }

    /// Poll until the server is ready to serve requests or the deadline elapses.
    pub async fn await_ready(&self, port: u16, deadline: Instant) -> Result<()> {
        match self {
            Self::LlamaCpp(adapter) => adapter.await_ready(port, deadline).await,
            Self::RapidMlx(adapter) => adapter.await_ready(port, deadline).await,
        }
    }

    /// Fetch a normalized metrics snapshot. Called by the shared poller loop.
    pub async fn poll_metrics(
        &self,
        port: u16,
        session_id: &str,
    ) -> Result<InferenceMetricsSnapshot> {
        match self {
            Self::LlamaCpp(adapter) => adapter.poll_metrics(port, session_id).await,
            Self::RapidMlx(adapter) => adapter.poll_metrics(port, session_id).await,
        }
    }

    /// Native request cancellation.
    pub async fn cancel_request(&self, port: u16, request_id: &str) -> Result<()> {
        match self {
            Self::LlamaCpp(adapter) => adapter.cancel_request(port, request_id).await,
            Self::RapidMlx(adapter) => adapter.cancel_request(port, request_id).await,
        }
    }

    /// Return the static capability set for the active runtime profile.
    pub fn capabilities(&self) -> &CapabilitySet {
        match self {
            Self::LlamaCpp(adapter) => adapter.capabilities(),
            Self::RapidMlx(adapter) => adapter.capabilities(),
        }
    }
}

#[cfg(test)]
mod recommendation_tests {
    use super::*;

    #[test]
    fn gguf_always_recommends_llama_cpp() {
        let result = recommend_backend(&BackendRecommendationInput {
            artifact_kind: RecommendationArtifactKind::Gguf,
            rapid_mlx_local_available: true,
            rapid_mlx_runtime_compatible: true,
        });
        assert_eq!(result.recommended_backend, Some(InferenceBackend::LlamaCpp));
        assert_eq!(result.state, "ready");
    }

    #[test]
    fn native_mlx_requires_platform_and_compatible_runtime() {
        let mut input = BackendRecommendationInput {
            artifact_kind: RecommendationArtifactKind::MlxDirectory,
            rapid_mlx_local_available: false,
            rapid_mlx_runtime_compatible: false,
        };
        assert_eq!(recommend_backend(&input).state, "platform_unavailable");

        input.rapid_mlx_local_available = true;
        assert_eq!(recommend_backend(&input).state, "runtime_required");

        input.rapid_mlx_runtime_compatible = true;
        assert_eq!(
            recommend_backend(&input).recommended_backend,
            Some(InferenceBackend::RapidMlx)
        );
    }

    #[test]
    fn unknown_sources_require_manual_selection() {
        let result = recommend_backend(&BackendRecommendationInput::default());
        assert_eq!(result.recommended_backend, None);
        assert_eq!(result.state, "manual_selection");
    }
}
