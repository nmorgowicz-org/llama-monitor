// Scenario fixture seeding (Rapid-MLX preset, nested-MLX model dir, fake .gguf models dir).
// Extracted from tests/ui/capture.mjs (Phase A1/A2).
import fs from 'fs';
import { join } from 'path';
import { TEMP_APP_CONFIG_DIR } from './paths.mjs';

export function seedRapidMlxCapturePreset() {
    const preset = [{
        id: 'capture-rapid-mlx',
        name: 'Qwen 3.6 · Rapid-MLX (legacy source)',
        backend: 'rapid_mlx',
        rapid_mlx: {
            model_path: 'mlx-community/Qwen3-30B-A3B-4bit',
            served_model_name: 'qwen3-rapid',
            host: '127.0.0.1',
            port: 9123,
            log_level: 'INFO',
            kv_cache_dtype: 'int4',
            reasoning_mode: 'on',
            enable_thinking: true,
            prefix_cache_enabled: true,
            retained_cache_mib: 8192,
            prefill_step_size: 512,
            turboquant_mode: 'none',
            tool_call_parser: '',
            reasoning_parser: '',
            workload_scenario: 'interactive_coding_agent',
            sampling_mode: 'coding',
            default_temperature: 0.7,
            default_top_p: 0.9,
        },
        model_path: '',
        port: 9123,
        context_size: 131072,
    }, {
        id: 'capture-rapid-mlx-typed',
        name: 'Qwen 3.6 · Rapid-MLX (typed source)',
        backend: 'rapid_mlx',
        rapid_mlx: {
            model_source: {
                kind: 'hugging_face_repo',
                repo_id: 'mlx-community/Qwen3.6-35B-A3B-4bit',
                revision: 'main',
            },
            served_model_name: 'qwen36-typed',
            host: '127.0.0.1',
            port: 9124,
            log_level: 'INFO',
            kv_cache_dtype: 'int4',
            reasoning_mode: 'on',
            enable_thinking: true,
            prefix_cache_enabled: true,
            retained_cache_mib: 8192,
            prefill_step_size: 512,
            turboquant_mode: 'none',
            tool_call_parser: '',
            reasoning_parser: '',
            workload_scenario: 'interactive_coding_agent',
            sampling_mode: 'coding',
            default_temperature: 0.7,
            default_top_p: 0.9,
        },
        model_path: '',
        port: 9124,
        context_size: 65536,
    }];
    fs.mkdirSync(TEMP_APP_CONFIG_DIR, { recursive: true });
    fs.writeFileSync(join(TEMP_APP_CONFIG_DIR, 'presets.json'), JSON.stringify(preset, null, 2));
}

export function seedNestedMlxFixture() {
    const fixtureDir = join(TEMP_APP_CONFIG_DIR, 'models', 'capture-nested-mlx');
    fs.mkdirSync(fixtureDir, { recursive: true });
    fs.writeFileSync(join(fixtureDir, 'config.json'), JSON.stringify({
        model_type: 'capture-wrapper',
        num_hidden_layers: 99,
        text_config: {
            model_type: 'qwen3_6',
            hidden_size: 1024,
            num_hidden_layers: 8,
            num_attention_heads: 8,
            num_key_value_heads: 2,
            head_dim: 128,
            full_attention_interval: 4,
            layer_types: ['full_attention', 'linear_attention', 'linear_attention', 'linear_attention', 'full_attention', 'linear_attention', 'linear_attention', 'linear_attention'],
            linear_key_head_dim: 64,
            linear_num_key_heads: 2,
        },
    }));
    fs.writeFileSync(join(fixtureDir, 'model.safetensors.index.json'), JSON.stringify({
        metadata: { total_size: 400_000_000 },
        weight_map: {},
    }));
}

export function seedModelsDirFixture() {
    const modelsDir = join(TEMP_APP_CONFIG_DIR, 'models');
    fs.mkdirSync(modelsDir, { recursive: true });
    const fakeFiles = [
        'Llama-3.3-70B-Instruct-Q4_K_M.gguf',
        'Qwen3-30B-A3B-Q5_K_M.gguf',
        'mistral-nemo-instruct-2407-Q4_K_M.gguf',
        'gemma-3-12b-it-Q8_0.gguf',
        'Devstral-Small-2-24B-UD-Q4_K_S.gguf',
        'Meta-Llama-3.1-8B-Instruct-i1-Q4_K_M.gguf',
    ];
    for (const f of fakeFiles) fs.writeFileSync(join(modelsDir, f), '');
    return ['--models-dir', modelsDir];
}
