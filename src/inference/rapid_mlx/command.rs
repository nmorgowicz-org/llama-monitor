use crate::inference::supervisor::SupervisedLaunch;
use std::ffi::OsString;
use std::path::PathBuf;

#[allow(dead_code)]
pub struct RapidMlxCommandBuilder {
    model_path: PathBuf,
    served_model_name: Option<String>,
    host: String,
    port: u16,
    log_level: String,
    max_num_seqs: u32,
    max_concurrent_requests: u32,
    prefill_batch_size: u32,
    completion_batch_size: u32,
    stream_interval: u32,
    max_tokens: u32,
    request_timeout: u32,
    gpu_memory_utilization: f32,
    paged_cache_block_size: u32,
    max_blocks: u32,
    prefill_step_size: u32,
}

#[allow(dead_code)]
impl RapidMlxCommandBuilder {
    pub fn new(model_path: PathBuf) -> Self {
        Self {
            model_path,
            served_model_name: None,
            host: "0.0.0.0".to_string(),
            port: 8000,
            log_level: "INFO".to_string(),
            max_num_seqs: 256,
            max_concurrent_requests: 256,
            prefill_batch_size: 8,
            completion_batch_size: 32,
            stream_interval: 1,
            max_tokens: 32768,
            request_timeout: 1800,
            gpu_memory_utilization: 0.90,
            paged_cache_block_size: 64,
            max_blocks: 1000,
            prefill_step_size: 2048,
        }
    }

    pub fn served_model_name(mut self, name: String) -> Self {
        self.served_model_name = Some(name);
        self
    }

    pub fn host(mut self, host: String) -> Self {
        self.host = host;
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn log_level(mut self, level: String) -> Self {
        self.log_level = level;
        self
    }

    pub fn max_num_seqs(mut self, seqs: u32) -> Self {
        self.max_num_seqs = seqs;
        self
    }

    pub fn max_concurrent_requests(mut self, requests: u32) -> Self {
        self.max_concurrent_requests = requests;
        self
    }

    pub fn prefill_batch_size(mut self, size: u32) -> Self {
        self.prefill_batch_size = size;
        self
    }

    pub fn completion_batch_size(mut self, size: u32) -> Self {
        self.completion_batch_size = size;
        self
    }

    pub fn stream_interval(mut self, interval: u32) -> Self {
        self.stream_interval = interval;
        self
    }

    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = tokens;
        self
    }

    pub fn request_timeout(mut self, timeout: u32) -> Self {
        self.request_timeout = timeout;
        self
    }

    pub fn gpu_memory_utilization(mut self, util: f32) -> Self {
        self.gpu_memory_utilization = util;
        self
    }

    pub fn paged_cache_block_size(mut self, size: u32) -> Self {
        self.paged_cache_block_size = size;
        self
    }

    pub fn max_blocks(mut self, blocks: u32) -> Self {
        self.max_blocks = blocks;
        self
    }

    pub fn prefill_step_size(mut self, size: u32) -> Self {
        self.prefill_step_size = size;
        self
    }

    pub fn build(self, binary_path: PathBuf) -> SupervisedLaunch {
        let mut args = vec!["serve".to_string()];
        args.push(self.model_path.to_string_lossy().into_owned());

        if let Some(name) = self.served_model_name {
            args.push("--served-model-name".to_string());
            args.push(name);
        }

        args.push("--host".to_string());
        args.push(self.host);

        args.push("--port".to_string());
        args.push(self.port.to_string());

        args.push("--log-level".to_string());
        args.push(self.log_level);

        args.push("--max-num-seqs".to_string());
        args.push(self.max_num_seqs.to_string());

        args.push("--max-concurrent-requests".to_string());
        args.push(self.max_concurrent_requests.to_string());

        args.push("--prefill-batch-size".to_string());
        args.push(self.prefill_batch_size.to_string());

        args.push("--completion-batch-size".to_string());
        args.push(self.completion_batch_size.to_string());

        args.push("--stream-interval".to_string());
        args.push(self.stream_interval.to_string());

        args.push("--max-tokens".to_string());
        args.push(self.max_tokens.to_string());

        args.push("--request-timeout".to_string());
        args.push(self.request_timeout.to_string());

        args.push("--gpu-memory-utilization".to_string());
        args.push(self.gpu_memory_utilization.to_string());

        args.push("--paged-cache-block-size".to_string());
        args.push(self.paged_cache_block_size.to_string());

        args.push("--max-blocks".to_string());
        args.push(self.max_blocks.to_string());

        args.push("--prefill-step-size".to_string());
        args.push(self.prefill_step_size.to_string());

        let os_args: Vec<OsString> = args.into_iter().map(OsString::from).collect();

        SupervisedLaunch {
            program: binary_path,
            args: os_args,
            env: Vec::new(),
            cwd: None,
            port: self.port,
            redacted_summary: format!(
                "Rapid-MLX serve: {} on port {}",
                self.model_path.display(),
                self.port
            ),
        }
    }
}
