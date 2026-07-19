# [E1] Template Applier Fact-Pin

Tag: [local-verifiable]
Phase: 0 (Phase 9 prerequisite)

## Question

Does Rapid-MLX (installed v0.10.10, audited v0.10.12) accept ANY `--chat-template`, `--chat-template-file`, or `--template-path` argument?

## Answer

NO. Rapid-MLX 0.10.10 (and 0.10.12 audited source) has no `--chat-template`, `--chat-template-file`, or template-path argument.

## Evidence

1. CLI help grep (installed v0.10.10):
   - Command: `rapid-mlx serve --help | grep -i "chat-template\|template-file\|template-path"`
   - Result: (no output)

2. Argument parser grep (server.py):
   - Command: `grep -n "add_argument.*chat.template\|add_argument.*template.file\|add_argument.*template.path" vllm_mlx/server.py`
   - Result: (no output)
   - Location: ~/.config/llama-monitor/runtimes/rapid-mlx/environments/0.10.10-abdd7d5421445014c012b23d/tool/rapid-mlx/lib/python3.14/site-packages/vllm_mlx/server.py

3. Argument parser grep (cli.py):
   - Command: `grep -n "add_argument.*chat.template\|add_argument.*template.file\|add_argument.*template.path" vllm_mlx/cli.py`
   - Result: (no output)
   - Location: ~/.config/llama-monitor/runtimes/rapid-mlx/environments/0.10.10-abdd7d5421445014c012b23d/tool/rapid-mlx/lib/python3.14/site-packages/vllm_mlx/cli.py

4. Reference to `chat_template_kwargs` in installed source:
   - cli.py:5663 notes: `chat_template_kwargs` is not a recognized server flag; only `enable_thinking` field is wired.
   - This forwards a narrow subset (thinking behavior) but does NOT replace the template.

## Conclusion

- Phase 9 Rapid template applier MUST use file placement (copy template into llama-monitor-owned model copy/overlay).
- It CANNOT be flag-based and symmetric with llama.cpp's `--chat-template-file`.
- Safety rule (for later phases): Rapid applier must NEVER mutate canonical/HF-cache model directory.

## CHECK

PASS iff: grep result recorded with file:line evidence showing absence of template-path args.
Status: PASS
