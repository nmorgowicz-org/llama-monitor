# Tuning Panel & Benchmarking

The Tuning panel provides access to server tuning settings, including sampling parameters and system-level tuning knobs. It is accessible via the **Tune** button in the Server tab header.

## Benchmark

The Benchmark feature runs a live throughput test against the active llama-server, grades the result, and returns actionable tuning suggestions.

### Benchmark Flow

The benchmark UI has three states:

1. **Idle**: A "Run Benchmark" button is displayed in the Tune panel.
2. **Running**: The button is disabled, a spinner is shown, and a hint line reads "Sending a test prompt and measuring throughput…".
3. **Results**: The grade chip, numeric results, and suggestion cards are displayed. A "Re-run" button allows re-testing after applying changes.

When the user clicks "Apply" on a suggestion card, the server is restarted with the modified configuration and the benchmark runs again automatically. Under the hood, the Tuning panel uses the spawn endpoint (`POST /api/sessions/spawn`) with admin authentication to perform a clean restart.

### Grade System

The generation throughput (`gen_tokens_per_second`) is mapped to a 5-tier letter grade:

| Grade | Minimum t/s | Label |
|-------|-------------|-------|
| **S** | 25 | Excellent |
| **A** | 12 | Good |
| **B** | 6 | Usable |
| **C** | 3 | Slow |
| **D** | 0 | Very Slow |

The grade chip appears in the results area with a color corresponding to the letter.

### Results

The benchmark sends a short test prompt through the server's chat completions endpoint and measures:

| Field | Description |
|-------|-------------|
| `gen_tokens_per_second` | Generation throughput (tokens/sec during decode) |
| `prompt_tokens_per_second` | Prefill throughput (tokens/sec during prompt processing) |
| `time_to_first_token_ms` | Time to first token in milliseconds |
| `verdict` | A classification of the result (e.g., "good", "bad") |

The backend sends a 512-token generation request with `temperature: 0.5` and `stream: true`.

### Suggestions

The response includes a `suggestions` array of tuning recommendations, each with a `label`, `description`, and target `param`/`value`.

Common suggestions include:
- **Enable flash attention** — when TTFT exceeds 1.5 s.
- **Try a smaller context window** — when gen t/s is below 5.
- **Increase batch size** — when prompt t/s is below 300.

### Cooldown

The benchmark endpoint enforces a 15-second cooldown to prevent repeated heavy loads on running llama-server. Attempts within the cooldown window return a `429 Too Many Requests` error.

---

## MTP (Multi-Token Prediction) Sweep

The MTP Sweep tool performs an empirical sweep to find the optimal configuration for Multi-Token Prediction (MTP) draft models.

### Sweep Operation

A sweep runs a series of probes by restarting the local server for each `n-max` value to measure how different draft configurations affect throughput and accuracy.

| Control | Description |
|----------|-------------|
| **Select Model** | Choose a model for the sweep (only works with supported architectures) |
| **n-max** | The maximum number of tokens to predict in a single sweep pass |
| **Sweep Results** | A summary table showing results for each probe |

### API

#### `POST /api/benchmark`

Requires `api-token`. Request body can be empty `{}`.

Response on success:

```json
{
  "prompt_tokens_per_second": 850.0,
  "gen_tokens_per_second": 15.3,
  "time_to_first_token_ms": 1200.0,
  "verdict": "good",
  "hints": ["String hint..."],
  "suggestions": [
    {
      "label": "Enable flash attention",
      "description": "Cuts time-to-first-token and reduces VRAM pressure at large context.",
      "param": "flash_attn",
      "value": "on",
      "patch": null
    }
  ]
}
```

Response on cooldown:

```json
{ "ok": false, "error": "Benchmark rate limited. Try again in 15 seconds.", "seconds_remaining": 8 }
```

---

## Tuning Cards

The tuning cards system is a shared card renderer used by the Tune Panel, Setup wizard performance advisor, and Preset Editor advisor.

### Card contract

Each suggestion follows this structure:

```json
{
  "label": "Enable flash attention",
  "description": "Cuts time-to-first-token and reduces VRAM pressure at large context.",
  "param": "flash_attn",
  "value": "on",
  "patch": null
}
```

| Field | Type | Description |
|-------|------|-------------|
| `label` | `string` | Short title displayed at the top of the card |
| `description` | `string` | Detailed explanation of the suggestion |
| `param` | `string` | Configuration key this suggestion modifies. Empty string (`""`) means the card is informational only |
| `value` | `any` | Target value to set for `param` |
| `patch` | `object \| null` | A multi-field object merged wholesale onto the config on Apply |

### Actionable vs Informational

- **Actionable cards**: Have an **Apply** button that modifies the configuration and restarts the server.
- **Informational cards**: `param` is an empty string. They provide context but have no Apply button.

---

## MoE (Mixture of Experts) Tuning

The Tuning panel includes specialized tools for optimizing MoE models.

### n_cpu_moe Autotuner

Estimates the optimal number of MoE layers to offload to CPU based on available VRAM and model architecture.

#### `POST /api/tune/ncpumoe`

Requires `api-token`. Request body:

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | `string` | No | `""` | Model name for architecture detection |
| `param_b` | `number` | No | `0` | Model size in billions of parameters |
| `model_size_bytes` | `number` | No | `0` | GGUF file size |
| `available_vram_bytes` | `number` | No | `0` | Available VRAM |
| `ubatch_size` | `number` | No | `512` | Unified batch size |
| `verify` | `boolean` | No | `false` | Run empirical llama-bench sweep |
| `model_path` | `string` | No | `""` | Path to GGUF (required for `verify: true`) |
| `ngl` | `number` | No | `99` | GPU layers |
| `ctk` | `string` | No | `"q8_0"` | Key cache type |
| `ctv` | `string` | No | `"q8_0"` | Value cache type |
| `flash_attn` | `boolean` | No | `true` | Flash attention |

Response:

```json
{
  "recommended_n_cpu_moe": 3,
  "verified": false
}
```

### Depth Sweep

A tool to analyze the impact of different context window depths on performance.

- Displays a summary table of results for various depths.
- Provides a "Spark" visualization of performance trends.
- Allows for comparing different depth configurations.

---

