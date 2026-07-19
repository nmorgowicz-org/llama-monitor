#!/usr/bin/env python3
"""One-shot detached-host runtime/fidelity gate for Phase 5.5 R3."""

import argparse
import json
import os
from pathlib import Path
import secrets
import signal
import socket
import subprocess
import sys
import threading
import time
from typing import Any
import urllib.error
import urllib.request


MAX_CAPTURE_BYTES = 64 * 1024
PROMPT = "Explain why the sky is blue in one short sentence."
RECOVERED_KEY = "a21cca76ec236c3c71ea2bf5eb6f78716602b90fb16d78c3aef4da51e1ff4177"
RECIPES = {
    "affine_4bit_g64": "bd494370cb354097bc67e714deb0f91d5ef6bb001e4cc8b4d695a0a6962e4522",
    "affine_6bit_g64": "40244bc4b24630c490a003a615d33a8c7705e12aec2268b7646e2ec81e4038ab",
    "affine_8bit_g64": "08f132594443be6efcfbb4b3d85c5c870e2c28048c06b702c16926e0384c5b29",
}


class GateError(RuntimeError):
    pass


class BoundedPipe:
    def __init__(self, pipe: Any):
        self.pipe = pipe
        self.retained = bytearray()
        self.total = 0
        self.thread = threading.Thread(target=self._drain, daemon=True)
        self.thread.start()

    def _drain(self) -> None:
        for chunk in iter(lambda: self.pipe.read(8192), b""):
            self.total += len(chunk)
            if len(self.retained) < MAX_CAPTURE_BYTES:
                keep = min(MAX_CAPTURE_BYTES - len(self.retained), len(chunk))
                self.retained.extend(chunk[:keep])

    def finish(self, timeout: float = 3.0) -> str:
        self.thread.join(timeout)
        if self.thread.is_alive():
            raise GateError("subprocess diagnostic pipe remained open")
        if self.total > MAX_CAPTURE_BYTES:
            raise GateError("subprocess diagnostic output exceeded 64 KiB")
        return self.retained.decode("utf-8", "replace")


def free_port() -> int:
    with socket.socket() as candidate:
        candidate.bind(("127.0.0.1", 0))
        return int(candidate.getsockname()[1])


def assert_port_closed(port: int) -> None:
    with socket.socket() as probe:
        probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        probe.bind(("127.0.0.1", port))


def request_json(
    url: str,
    *,
    api_key: str | None = None,
    payload: dict[str, Any] | None = None,
    timeout: float = 10.0,
) -> dict[str, Any]:
    headers = {"Accept": "application/json"}
    data = None
    if api_key:
        headers["Authorization"] = f"Bearer {api_key}"
    if payload is not None:
        headers["Content-Type"] = "application/json"
        data = json.dumps(payload, separators=(",", ":")).encode()
    request = urllib.request.Request(url, data=data, headers=headers)
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            body = response.read(1024 * 1024 + 1)
    except urllib.error.HTTPError as error:
        detail = error.read(4096).decode("utf-8", "replace")
        raise GateError(f"HTTP {error.code} from {url}: {detail}") from error
    if len(body) > 1024 * 1024:
        raise GateError(f"HTTP response exceeded 1 MiB: {url}")
    value = json.loads(body)
    if not isinstance(value, dict):
        raise GateError(f"HTTP response was not an object: {url}")
    return value


def wait_ready(
    port: int,
    process: subprocess.Popen[bytes],
    timeout: float = 60.0,
    path: str = "/health/ready",
) -> dict[str, Any]:
    deadline = time.monotonic() + timeout
    last_error = "not started"
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise GateError(f"server exited before readiness with {process.returncode}")
        try:
            ready = request_json(f"http://127.0.0.1:{port}{path}", timeout=2)
            if ready.get("ready") is True or ready.get("status") == "ok":
                return ready
        except Exception as error:
            last_error = str(error)
        time.sleep(0.2)
    raise GateError(f"server readiness timed out: {last_error}")


def stop_server(process: subprocess.Popen[bytes], port: int) -> tuple[str, str]:
    if process.poll() is None:
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait(timeout=5)
    # The group can outlive a reaped leader. Always close that ownership boundary
    # before joining inherited pipes or declaring the listener gone.
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    stdout = process._r3_stdout.finish()  # type: ignore[attr-defined]
    stderr = process._r3_stderr.finish()  # type: ignore[attr-defined]
    assert_port_closed(port)
    return stdout, stderr


def start_server(command: list[str], environment: dict[str, str]) -> subprocess.Popen[bytes]:
    process = subprocess.Popen(
        command,
        env=environment,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    if process.stdout is None or process.stderr is None:
        raise GateError("server diagnostic pipes unavailable")
    process._r3_stdout = BoundedPipe(process.stdout)  # type: ignore[attr-defined]
    process._r3_stderr = BoundedPipe(process.stderr)  # type: ignore[attr-defined]
    return process


def run_json(command: list[str], environment: dict[str, str], timeout: float = 180.0) -> dict[str, Any]:
    started = time.monotonic()
    completed = subprocess.run(
        command,
        env=environment,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
        check=False,
    )
    if len(completed.stdout) > MAX_CAPTURE_BYTES or len(completed.stderr) > MAX_CAPTURE_BYTES:
        raise GateError("bounded command exceeded diagnostic output limit")
    if completed.returncode != 0:
        raise GateError(
            f"command failed with {completed.returncode}: "
            f"{completed.stderr.decode('utf-8', 'replace')[:2000]}"
        )
    value = json.loads(completed.stdout)
    if not isinstance(value, dict):
        raise GateError("bounded command did not return a JSON object")
    value["wall_seconds"] = time.monotonic() - started
    return value


def load_probe(python: Path, model: Path, environment: dict[str, str]) -> dict[str, Any]:
    script = (
        "import json,time; from mlx_lm import load; "
        f"p={str(model)!r}; s=time.monotonic(); m,t=load(p); "
        "print(json.dumps({'model_class':type(m).__name__,'tokenizer_class':type(t).__name__,"
        "'load_seconds':time.monotonic()-s}))"
    )
    return run_json([str(python), "-I", "-c", script], environment, timeout=120)


def top_logprobs(response: dict[str, Any]) -> list[dict[str, Any]]:
    try:
        return response["choices"][0]["logprobs"]["content"][0]["top_logprobs"]
    except (KeyError, IndexError, TypeError):
        return []


def compare_logprobs(
    reference: list[dict[str, Any]], candidate: list[dict[str, Any]]
) -> dict[str, Any]:
    reference_tokens = [item.get("token") for item in reference]
    candidate_tokens = [item.get("token") for item in candidate]
    reference_values = {item.get("token"): item.get("logprob") for item in reference}
    candidate_values = {item.get("token"): item.get("logprob") for item in candidate}
    shared = set(reference_values) & set(candidate_values)
    deltas = [
        abs(float(reference_values[token]) - float(candidate_values[token]))
        for token in shared
        if reference_values[token] is not None and candidate_values[token] is not None
    ]
    return {
        "reference_tokens": reference_tokens,
        "candidate_tokens": candidate_tokens,
        "ordered_tokens_match": candidate_tokens == reference_tokens,
        "winning_token_matches": bool(candidate_tokens)
        and bool(reference_tokens)
        and candidate_tokens[0] == reference_tokens[0],
        "shared_token_count": len(shared),
        "max_shared_logprob_delta": max(deltas) if deltas else None,
    }


def chat_text(response: dict[str, Any]) -> str:
    try:
        value = response["choices"][0]["message"]["content"]
    except (KeyError, IndexError, TypeError) as error:
        raise GateError("chat response has no assistant content") from error
    if not isinstance(value, str) or not value.strip():
        raise GateError("chat response is empty")
    printable = sum(character.isprintable() or character.isspace() for character in value)
    if printable / len(value) < 0.95:
        raise GateError("chat response is not predominantly printable text")
    return value


def chat_payload(model_name: str) -> dict[str, Any]:
    return {
        "model": model_name,
        "messages": [{"role": "user", "content": PROMPT}],
        "temperature": 0,
        "seed": 55,
        "max_tokens": 32,
        "stream": False,
        "logprobs": True,
        "top_logprobs": 5,
    }


def rapid_probe(
    rapid: Path,
    model: Path,
    label: str,
    environment: dict[str, str],
) -> dict[str, Any]:
    port = free_port()
    api_key = secrets.token_urlsafe(24)
    process = start_server(
        [
            str(rapid),
            "serve",
            str(model),
            "--served-model-name",
            label,
            "--host",
            "127.0.0.1",
            "--port",
            str(port),
            "--timeout",
            "60",
            "--max-tokens",
            "64",
            "--log-level",
            "INFO",
        ],
        {**environment, "RAPID_MLX_API_KEY": api_key},
    )
    result: dict[str, Any] = {"port": port}
    try:
        result["readiness"] = wait_ready(port, process)
        base = f"http://127.0.0.1:{port}"
        result["models"] = request_json(f"{base}/v1/models", api_key=api_key)
        model_ids = [
            item.get("id")
            for item in result["models"].get("data", [])
            if isinstance(item, dict)
        ]
        if model_ids != [label]:
            raise GateError(f"{label} returned an unexpected model identity: {model_ids}")
        result["status_before"] = request_json(f"{base}/v1/status", api_key=api_key)
        if result["status_before"].get("model") != label:
            raise GateError(f"{label} status returned an unexpected model identity")
        result["cache_before"] = request_json(f"{base}/v1/cache/stats", api_key=api_key)
        responses = []
        for _ in range(2):
            started = time.monotonic()
            response = request_json(
                f"{base}/v1/chat/completions",
                api_key=api_key,
                payload=chat_payload(label),
                timeout=90,
            )
            elapsed = time.monotonic() - started
            usage = response.get("usage", {})
            prompt_tokens = int(usage.get("prompt_tokens", 0))
            completion_tokens = int(usage.get("completion_tokens", 0))
            candidates = top_logprobs(response)
            if len(candidates) != 5:
                raise GateError(f"{label} did not return exactly five first-token logprobs")
            finish_reason = response.get("choices", [{}])[0].get("finish_reason")
            if finish_reason not in {"stop", "length"}:
                raise GateError(f"{label} returned an invalid finish reason")
            if prompt_tokens <= 0 or completion_tokens <= 0:
                raise GateError(f"{label} returned invalid token usage")
            responses.append(
                {
                    "text": chat_text(response),
                    "finish_reason": finish_reason,
                    "usage": usage,
                    "elapsed_seconds": elapsed,
                    "prompt_tokens_per_wall_second": prompt_tokens / elapsed if elapsed else 0,
                    "completion_tokens_per_second": completion_tokens / elapsed if elapsed else 0,
                    "top_logprobs": candidates,
                }
            )
        result["responses"] = responses
        result["deterministic"] = responses[0]["text"] == responses[1]["text"]
        if not result["deterministic"]:
            raise GateError(f"{label} repeated greedy output differed")
        result["status_after"] = request_json(f"{base}/v1/status", api_key=api_key)
        result["cache_after"] = request_json(f"{base}/v1/cache/stats", api_key=api_key)
        return result
    finally:
        stdout, stderr = stop_server(process, port)
        result["diagnostics"] = {
            "stdout": stdout.replace(api_key, "[REDACTED]"),
            "stderr": stderr.replace(api_key, "[REDACTED]"),
        }


def llama_probe(llama: Path, gguf: Path, environment: dict[str, str]) -> dict[str, Any]:
    port = free_port()
    process = start_server(
        [
            str(llama),
            "-m",
            str(gguf),
            "--host",
            "127.0.0.1",
            "--port",
            str(port),
            "--jinja",
        ],
        environment,
    )
    result: dict[str, Any] = {"port": port}
    try:
        result["readiness"] = wait_ready(port, process, path="/health")
        started = time.monotonic()
        response = request_json(
            f"http://127.0.0.1:{port}/v1/chat/completions",
            payload=chat_payload("r3-llama-f16-reference"),
            timeout=90,
        )
        elapsed = time.monotonic() - started
        usage = response.get("usage", {})
        prompt_tokens = int(usage.get("prompt_tokens", 0))
        completion_tokens = int(usage.get("completion_tokens", 0))
        candidates = top_logprobs(response)
        if len(candidates) != 5:
            raise GateError("llama-server did not return exactly five first-token logprobs")
        finish_reason = response.get("choices", [{}])[0].get("finish_reason")
        if finish_reason not in {"stop", "length"}:
            raise GateError("llama-server returned an invalid finish reason")
        if prompt_tokens <= 0 or completion_tokens <= 0:
            raise GateError("llama-server returned invalid token usage")
        result["response"] = {
            "text": chat_text(response),
            "finish_reason": finish_reason,
            "usage": usage,
            "elapsed_seconds": elapsed,
            "prompt_tokens_per_wall_second": prompt_tokens / elapsed if elapsed else 0,
            "completion_tokens_per_second": completion_tokens / elapsed if elapsed else 0,
            "top_logprobs": candidates,
        }
        return result
    finally:
        stdout, stderr = stop_server(process, port)
        result["diagnostics"] = {"stdout": stdout, "stderr": stderr}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", default="/tmp/llama-monitor-r3-host-gate/report.json")
    args = parser.parse_args()
    home = Path.home()
    config = home / ".config/llama-monitor"
    models = config / "models"
    runtime = config / "runtimes/rapid-mlx/.staging/0.10.10-qualification/venv"
    python = runtime / "bin/python"
    rapid = runtime / "bin/rapid-mlx"
    llama = config / "bin/llama-server"
    recovered = models / "rapid-mlx/imports" / RECOVERED_KEY / "fp16"
    gguf = (
        models
        / "experimental/import-lab/fixtures/smollm2-135m-v1/gguf/SmolLM2-135M-Instruct-F16.gguf"
    )
    repo = Path(__file__).resolve().parents[2]
    fidelity = repo / "tools/mlx_requantize/validate_fidelity.py"
    environment = {
        "PATH": "",
        "PYTHONNOUSERSITE": "1",
        "HF_HUB_OFFLINE": "1",
        "TRANSFORMERS_OFFLINE": "1",
        "HF_HUB_DISABLE_TELEMETRY": "1",
        "RAPID_MLX_TELEMETRY": "0",
    }
    for required in [python, rapid, llama, recovered, gguf, fidelity]:
        if not required.exists():
            raise GateError(f"required pinned input is missing: {required}")
    report: dict[str, Any] = {
        "schema_version": 1,
        "prompt": PROMPT,
        "recovered_cache_key": RECOVERED_KEY,
        "human_coherence_review_required": True,
        "models": {},
    }
    report["llama_f16_reference"] = llama_probe(llama, gguf, environment)
    all_models = {"recovered_fp16": recovered}
    all_models.update(
        {
            recipe: models / "rapid-mlx/requantized" / cache_key / "model"
            for recipe, cache_key in RECIPES.items()
        }
    )
    for label, model in all_models.items():
        entry: dict[str, Any] = {
            "path": str(model),
            "size_bytes": sum(path.stat().st_size for path in model.rglob("*") if path.is_file()),
            "load": load_probe(python, model, environment),
            "rapid_mlx": rapid_probe(rapid, model, f"r3-{label}", environment),
        }
        if label != "recovered_fp16":
            entry["fidelity"] = run_json(
                [
                    str(python),
                    "-I",
                    str(fidelity),
                    "--source",
                    str(recovered / "model.safetensors"),
                    "--quantized",
                    str(model),
                    "--recipe",
                    label,
                    "--limit",
                    "8",
                ],
                environment,
                timeout=180,
            )
        report["models"][label] = entry
    recovered_top = report["models"]["recovered_fp16"]["rapid_mlx"]["responses"][0][
        "top_logprobs"
    ]
    llama_top = report["llama_f16_reference"]["response"]["top_logprobs"]
    for label, entry in report["models"].items():
        candidates = entry["rapid_mlx"]["responses"][0]["top_logprobs"]
        entry["logprob_compatibility"] = {
            "vs_recovered_fp16": compare_logprobs(recovered_top, candidates),
            "vs_llama_f16_gguf": compare_logprobs(llama_top, candidates),
        }
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_suffix(".tmp")
    encoded = (json.dumps(report, sort_keys=True, indent=2) + "\n").encode()
    if len(encoded) > 2 * 1024 * 1024:
        raise GateError("host-gate report exceeds 2 MiB")
    with temporary.open("xb") as target:
        target.write(encoded)
        target.flush()
        os.fsync(target.fileno())
    temporary.replace(output)
    print(json.dumps({"status": "completed", "report": str(output)}, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(json.dumps({"status": "failed", "error": str(error)[:2000]}), file=sys.stderr)
        raise SystemExit(1)
