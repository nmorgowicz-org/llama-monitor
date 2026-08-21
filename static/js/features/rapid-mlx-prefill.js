// Rapid-MLX prefill defaults shared by the Spawn Wizard and Preset Editor.
// Text-only workloads stay at 512; verified model vision lanes start at 1536
// so ordinary UI screenshots fit the runtime's per-batch image-token cap.

export const RAPID_MLX_TEXT_PREFILL_STEP_SIZE = 512;
export const RAPID_MLX_VISION_PREFILL_STEP_SIZE = 1536;
export const RAPID_MLX_DEFAULT_SPECULATIVE_TOKENS = 3;

export function rapidMlxProfileHasVision(profile) {
    return profile?.extras?.has_vision_tower === true;
}

export function rapidMlxPrefillStepSizeDefault(profile) {
    return rapidMlxProfileHasVision(profile)
        ? RAPID_MLX_VISION_PREFILL_STEP_SIZE
        : RAPID_MLX_TEXT_PREFILL_STEP_SIZE;
}
