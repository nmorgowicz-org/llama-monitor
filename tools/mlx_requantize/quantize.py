#!/usr/bin/env python3
"""Strict offline Phase 5.5 R3 adapter around official mlx-lm quantization."""

import argparse
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import shutil
import sys
import time
from typing import Any

import numpy as np
from safetensors import safe_open


WORKER_VERSION = "llama-monitor-mlx-requantize-r3-v1"
MAX_JSON_BYTES = 1024 * 1024


class QuantizeError(RuntimeError):
    pass


def bounded(value: object, limit: int = 1000) -> str:
    return str(value).replace("\n", " ")[:limit]


def read_json(path: Path, limit: int = MAX_JSON_BYTES) -> dict[str, Any]:
    if path.is_symlink() or not path.is_file() or path.stat().st_size > limit:
        raise QuantizeError(f"Invalid or oversized JSON file: {path.name}")
    value = json.loads(path.read_bytes())
    if not isinstance(value, dict):
        raise QuantizeError(f"JSON root must be an object: {path.name}")
    return value


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_directory(value: str, kind: str) -> Path:
    path = Path(value)
    if path.is_symlink():
        raise QuantizeError(f"{kind} must not be a symlink")
    resolved = path.resolve(strict=True)
    if not resolved.is_dir():
        raise QuantizeError(f"{kind} must be a directory")
    return resolved


def require_child(path: Path, root: Path, kind: str) -> None:
    if path == root or root not in path.parents:
        raise QuantizeError(f"{kind} escapes its staging root")


def check_cancel(path: Path) -> None:
    if path.exists():
        raise QuantizeError("Re-quantization cancelled")


def flat_files(root: Path) -> dict[str, str]:
    result: dict[str, str] = {}
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            raise QuantizeError("Symlink found in re-quantized output")
        if path.is_file():
            result[path.relative_to(root).as_posix()] = sha256_file(path)
        elif not path.is_dir():
            raise QuantizeError("Unsupported output entry")
    return result


def output_bytes(root: Path) -> int:
    return sum(path.stat().st_size for path in root.rglob("*") if path.is_file())


def safetensor_inventory(root: Path) -> tuple[list[dict[str, Any]], int, int]:
    inventory: list[dict[str, Any]] = []
    scales = 0
    biases = 0
    seen: set[str] = set()
    files = sorted(root.glob("*.safetensors"))
    if not files:
        raise QuantizeError("No safetensors output was produced")
    for path in files:
        with safe_open(path, framework="np") as source:
            for key in source.keys():
                if key in seen:
                    raise QuantizeError(f"Duplicate output tensor: {key}")
                seen.add(key)
                view = source.get_slice(key)
                dtype = view.get_dtype()
                shape = [int(value) for value in view.get_shape()]
                if key.endswith(".scales"):
                    scales += 1
                elif key.endswith(".biases"):
                    biases += 1
                if dtype in {"F16", "BF16", "F32", "F64"}:
                    try:
                        value = source.get_tensor(key)
                    except TypeError as error:
                        if "bfloat16" in str(error):
                            value = None
                        else:
                            raise
                    if value is not None and not np.isfinite(value).all():
                        raise QuantizeError(f"Non-finite output tensor: {key}")
                inventory.append({"name": key, "dtype": dtype, "shape": shape})
    inventory.sort(key=lambda item: item["name"])
    if scales == 0 or biases == 0 or scales != biases:
        raise QuantizeError("Quantized scale/bias tensor closure is invalid")
    return inventory, scales, biases


def validate_source(source: Path, profile: dict[str, Any]) -> dict[str, Any]:
    contract = profile["source"]
    manifest = read_json(source.parent / "manifest.json")
    report = read_json(source.parent / "validation.json")
    if manifest.get("cache_key") != contract["recovery_cache_key"]:
        raise QuantizeError("Recovered source cache key differs from profile")
    if manifest.get("profile_id") != contract["recovery_profile_id"]:
        raise QuantizeError("Recovered source profile differs from R3 profile")
    if manifest.get("source_tier") != contract["original_quant_tier"]:
        raise QuantizeError("Original GGUF tier differs from R3 profile")
    if manifest.get("launchable") is not False:
        raise QuantizeError("Recovered R2 source must remain non-launchable")
    if sha256_file(source.parent / "manifest.json") != contract["recovery_manifest_sha256"]:
        raise QuantizeError("Recovered R2 manifest hash differs from profile")
    if sha256_file(source.parent / "validation.json") != contract["recovery_report_sha256"]:
        raise QuantizeError("Recovered R2 report hash differs from profile")
    weight = source / "model.safetensors"
    if sha256_file(weight) != contract["recovered_weight_sha256"]:
        raise QuantizeError("Recovered FP16 weight hash differs from profile")
    if report.get("output", {}).get("tensor_count") != contract["recovered_tensor_count"]:
        raise QuantizeError("Recovered FP16 tensor count differs from profile")
    return manifest


def validate_output(
    output: Path, source: Path, recipe_name: str, recipe: dict[str, Any]
) -> dict[str, Any]:
    config = read_json(output / "config.json")
    expected = {
        "group_size": recipe["group_size"],
        "bits": recipe["bits"],
        "mode": recipe["mode"],
    }
    if config.get("quantization") != expected or config.get("quantization_config") != expected:
        raise QuantizeError("Output quantization config differs from exact recipe")
    for name in [
        "generation_config.json",
        "merges.txt",
        "special_tokens_map.json",
        "tokenizer.json",
        "tokenizer_config.json",
        "vocab.json",
    ]:
        candidate = output / name
        original = source / name
        if not candidate.is_file():
            present = ",".join(sorted(path.name for path in output.iterdir()))
            raise QuantizeError(
                f"Tokenizer/generation asset is missing: {name}; output_files={present}"
            )
        actual_hash = sha256_file(candidate)
        expected_hash = sha256_file(original)
        if actual_hash != expected_hash:
            raise QuantizeError(
                f"Tokenizer/generation asset differs from recovered FP16: {name} "
                f"expected={expected_hash} actual={actual_hash} "
                f"expected_size={original.stat().st_size} actual_size={candidate.stat().st_size}"
            )
    inventory, scales, biases = safetensor_inventory(output)
    return {
        "recipe_id": recipe_name,
        "mode": recipe["mode"],
        "bits": recipe["bits"],
        "group_size": recipe["group_size"],
        "tensor_count": len(inventory),
        "quantized_module_count": scales,
        "scale_tensor_count": scales,
        "bias_tensor_count": biases,
        "tensor_inventory_sha256": hashlib.sha256(
            json.dumps(inventory, separators=(",", ":")).encode()
        ).hexdigest(),
        "tensor_inventory": inventory,
    }


def restore_exact_source_assets(output: Path, source: Path) -> None:
    """Keep official weight/config conversion while preserving R2 tokenizer identity."""
    for generated_only in ["README.md", "chat_template.jinja"]:
        candidate = output / generated_only
        if candidate.exists():
            if candidate.is_symlink() or not candidate.is_file():
                raise QuantizeError(f"Unexpected generated asset type: {generated_only}")
            candidate.unlink()
    for name in [
        "generation_config.json",
        "merges.txt",
        "special_tokens_map.json",
        "tokenizer.json",
        "tokenizer_config.json",
        "vocab.json",
    ]:
        original = source / name
        candidate = output / name
        if candidate.exists() and (candidate.is_symlink() or not candidate.is_file()):
            raise QuantizeError(f"Unexpected tokenizer asset type: {name}")
        shutil.copyfile(original, candidate)


def atomic_report(path: Path, report: dict[str, Any]) -> None:
    encoded = (json.dumps(report, sort_keys=True, indent=2) + "\n").encode()
    if len(encoded) > MAX_JSON_BYTES:
        raise QuantizeError("Worker report exceeds bound")
    temporary = path.with_suffix(".tmp")
    with temporary.open("xb") as target:
        target.write(encoded)
        target.flush()
        os.fsync(target.fileno())
    temporary.replace(path)


def run(request: dict[str, Any]) -> dict[str, Any]:
    if request.get("schema_version") != 1:
        raise QuantizeError("Unsupported request schema")
    profile = read_json(Path(request["profile_path"]))
    if profile.get("profile_id") != request.get("profile_id"):
        raise QuantizeError("Request/profile identity mismatch")
    recipe_name = request.get("recipe_id")
    recipe = profile.get("recipes", {}).get(recipe_name)
    if not isinstance(recipe, dict):
        raise QuantizeError("Unknown re-quantization recipe")
    source = canonical_directory(request["source_fp16"], "recovered FP16 source")
    staging = canonical_directory(request["staging_root"], "staging root")
    output = Path(request["output_dir"])
    require_child(output, staging, "output")
    if output.exists() or output.is_symlink():
        raise QuantizeError("Output path must be new")
    cancel = Path(request["cancel_path"])
    require_child(cancel, staging, "cancellation sentinel")
    check_cancel(cancel)
    source_manifest = validate_source(source, profile)
    runtime = profile["runtime"]
    if importlib.metadata.version("mlx-lm") != runtime["mlx_lm_version"]:
        raise QuantizeError("mlx-lm version differs from profile")
    if importlib.metadata.version("mlx") != runtime["mlx_version"]:
        raise QuantizeError("MLX version differs from profile")
    max_output = int(request["max_output_bytes"])
    if max_output <= 0:
        raise QuantizeError("Output bound must be positive")
    source_weight_bytes = (source / "model.safetensors").stat().st_size
    conservative_estimate = source_weight_bytes + 16 * 1024 * 1024
    if conservative_estimate > max_output:
        raise QuantizeError("Conservative output estimate exceeds request bound")
    free = shutil.disk_usage(staging).free
    if free < conservative_estimate + int(request.get("disk_safety_margin_bytes", 0)):
        raise QuantizeError("Insufficient free space for re-quantization")
    check_cancel(cancel)
    started = time.monotonic()
    from mlx_lm.convert import convert

    convert(
        hf_path=str(source),
        mlx_path=str(output),
        quantize=True,
        q_group_size=int(recipe["group_size"]),
        q_bits=int(recipe["bits"]),
        q_mode=str(recipe["mode"]),
        dtype="float16",
        trust_remote_code=False,
    )
    check_cancel(cancel)
    restore_exact_source_assets(output, source)
    check_cancel(cancel)
    output_contract = validate_output(output, source, recipe_name, recipe)
    files = flat_files(output)
    actual_bytes = output_bytes(output)
    if actual_bytes > max_output:
        raise QuantizeError("Actual re-quantized output exceeds request bound")
    check_cancel(cancel)
    return {
        "schema_version": 1,
        "worker_version": WORKER_VERSION,
        "status": "requantized",
        "profile_id": profile["profile_id"],
        "labels": {
            "original_format": profile["source"]["original_format"],
            "original_gguf_quant_tier": profile["source"]["original_quant_tier"],
            "recovery_format": profile["source"]["recovery_format"],
            "recovery_cache_key": profile["source"]["recovery_cache_key"],
            "output_format": "mlx_quantized_safetensors",
            "output_recipe": recipe_name,
        },
        "source": {
            "path": str(source),
            "cache_key": source_manifest["cache_key"],
            "manifest_sha256": profile["source"]["recovery_manifest_sha256"],
            "report_sha256": profile["source"]["recovery_report_sha256"],
            "weight_sha256": profile["source"]["recovered_weight_sha256"],
            "tensor_count": profile["source"]["recovered_tensor_count"],
        },
        "runtime": {
            "mlx_lm_version": runtime["mlx_lm_version"],
            "mlx_version": runtime["mlx_version"],
        },
        "recipe": output_contract,
        "output": {
            "files": files,
            "actual_output_bytes": actual_bytes,
        },
        "elapsed_seconds": time.monotonic() - started,
        "launchable": False,
        "error": "",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--request", required=True)
    args = parser.parse_args()
    request_path = Path(args.request)
    request: dict[str, Any] = {}
    report_path: Path | None = None
    output: Path | None = None
    try:
        request = read_json(request_path)
        report_path = Path(request["report_path"])
        output = Path(request["output_dir"])
        report = run(request)
        atomic_report(report_path, report)
        return 0
    except Exception as error:
        if output is not None and output.exists() and not output.is_symlink():
            shutil.rmtree(output, ignore_errors=True)
        failure = {
            "schema_version": 1,
            "worker_version": WORKER_VERSION,
            "status": "failed",
            "error": bounded(error),
            "launchable": False,
        }
        if report_path is not None:
            try:
                atomic_report(report_path, failure)
            except Exception:
                pass
        print(json.dumps(failure), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
