#!/usr/bin/env python3
"""Strict profile-scoped GGUF recovery worker for Phase 5.5 R2.

Derived from the GGUF reading/dequantization and Llama mapping concepts audited at
barrontang/gguf2mlx@6a0da6529f233df79362cbf62dd96221c895351f. See
THIRD_PARTY_NOTICE.md. This worker has no network behavior and no architecture fallback.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import shutil
import sys
import traceback
from pathlib import Path
from typing import Any

import numpy as np
from gguf import GGUFReader
from gguf.constants import GGMLQuantizationType
from gguf.quants import dequantize
from safetensors import safe_open
from safetensors.numpy import save_file

WORKER_VERSION = "llama-monitor-gguf-recovery-r2-v1"
REPORT_SCHEMA_VERSION = 1
MAX_REQUEST_BYTES = 64 * 1024
MAX_ERROR_CHARS = 4096


class RecoveryError(RuntimeError):
    pass


class Cancelled(RecoveryError):
    pass


def bounded(value: Any, limit: int = MAX_ERROR_CHARS) -> str:
    text = str(value).replace("\x00", "")
    return text[:limit]


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()

def canonical_json_sha256(value: Any) -> str:
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()

def flat_directory_bytes(path: Path) -> int:
    total = 0
    for item in path.iterdir():
        if item.is_symlink() or not item.is_file():
            raise RecoveryError(f"Unexpected non-file in recovered output: {item.name}")
        total += item.stat().st_size
    return total


def canonical_existing(path: str, kind: str) -> Path:
    raw = Path(path)
    if not raw.is_absolute():
        raise RecoveryError(f"{kind} must be absolute")
    resolved = raw.resolve(strict=True)
    if raw.is_symlink():
        raise RecoveryError(f"{kind} must not be a symlink")
    return resolved


def require_child(path: Path, root: Path, kind: str) -> None:
    try:
        path.relative_to(root)
    except ValueError as exc:
        raise RecoveryError(f"{kind} escapes its canonical root") from exc


def read_json(path: Path, maximum: int) -> dict[str, Any]:
    if path.stat().st_size > maximum:
        raise RecoveryError(f"JSON file exceeds {maximum}-byte bound: {path.name}")
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise RecoveryError(f"Expected JSON object: {path.name}")
    return value


def metadata_str(reader: GGUFReader, key: str) -> str | None:
    field = reader.get_field(key)
    if field is None:
        return None
    value = field.contents()
    if isinstance(value, bytes):
        return value.decode("utf-8", errors="strict")
    return str(value) if value is not None else None


def qtype_name(value: int) -> str:
    try:
        return GGMLQuantizationType(value).name
    except ValueError as exc:
        raise RecoveryError(f"Unknown GGML quantization type {value}") from exc


def map_llama_name(name: str, layers: int) -> str:
    roots = {
        "token_embd.weight": "model.embed_tokens.weight",
        "output.weight": "lm_head.weight",
        "output_norm.weight": "model.norm.weight",
    }
    if name in roots:
        return roots[name]
    parts = name.split(".", 2)
    if len(parts) != 3 or parts[0] != "blk" or not parts[1].isdigit():
        raise RecoveryError(f"Unmapped source tensor: {name}")
    layer = int(parts[1])
    if layer >= layers:
        raise RecoveryError(f"Tensor layer {layer} exceeds profile layer count {layers}")
    suffixes = {
        "attn_q.weight": "self_attn.q_proj.weight",
        "attn_k.weight": "self_attn.k_proj.weight",
        "attn_v.weight": "self_attn.v_proj.weight",
        "attn_output.weight": "self_attn.o_proj.weight",
        "ffn_gate.weight": "mlp.gate_proj.weight",
        "ffn_up.weight": "mlp.up_proj.weight",
        "ffn_down.weight": "mlp.down_proj.weight",
        "attn_norm.weight": "input_layernorm.weight",
        "ffn_norm.weight": "post_attention_layernorm.weight",
    }
    mapped = suffixes.get(parts[2])
    if mapped is None:
        raise RecoveryError(f"Unmapped source tensor: {name}")
    return f"model.layers.{layer}.{mapped}"


def inverse_rope_permute(array: np.ndarray, heads: int) -> np.ndarray:
    if array.ndim != 2 or array.shape[0] % heads != 0:
        raise RecoveryError(
            f"RoPE inverse requires a 2D tensor divisible by {heads} heads, got {array.shape}"
        )
    head_dim = array.shape[0] // heads
    if head_dim % 2:
        raise RecoveryError(f"RoPE head dimension must be even, got {head_dim}")
    return array.reshape(heads, head_dim // 2, 2, array.shape[1]).swapaxes(1, 2).reshape(array.shape)


def recover_array(tensor: Any, output_name: str, config: dict[str, Any]) -> np.ndarray:
    qtype_value = int(tensor.tensor_type)
    if qtype_value == 0:
        # gguf 0.19 exposes unquantized tensor.data in logical ML/HF order already.
        # Reshaping through tensor.shape (GGML dimension order) scrambles the buffer.
        array = np.asarray(tensor.data, dtype=np.float32).astype(np.float16)
    elif qtype_value == 1:
        array = np.asarray(tensor.data, dtype=np.float16)
    else:
        try:
            array = dequantize(tensor.data, GGMLQuantizationType(qtype_value)).astype(np.float16)
        except Exception as exc:
            raise RecoveryError(
                f"Failed to dequantize {tensor.name} ({qtype_name(qtype_value)}): {bounded(exc)}"
            ) from exc
    if output_name.endswith("self_attn.q_proj.weight"):
        array = inverse_rope_permute(array, int(config["num_attention_heads"]))
    elif output_name.endswith("self_attn.k_proj.weight"):
        array = inverse_rope_permute(array, int(config["num_key_value_heads"]))
    if not np.isfinite(array).all():
        raise RecoveryError(f"Non-finite values in recovered tensor {output_name}")
    return np.ascontiguousarray(array, dtype=np.float16)


def authoritative_inventory(reference_weight: Path) -> dict[str, tuple[int, ...]]:
    result: dict[str, tuple[int, ...]] = {}
    with safe_open(reference_weight, framework="np") as source:
        for key in source.keys():
            if key in result:
                raise RecoveryError(f"Duplicate authoritative tensor {key}")
            result[key] = tuple(int(value) for value in source.get_slice(key).get_shape())
    return result


def validate_config(config: dict[str, Any], contract: dict[str, Any]) -> None:
    for key, expected in contract.items():
        if key not in config:
            raise RecoveryError(f"Authoritative config is missing required field {key}")
        if config[key] != expected:
            raise RecoveryError(
                f"Authoritative config field {key} differs from profile: {config[key]!r} != {expected!r}"
            )


def validate_reference(
    reference: Path, profile: dict[str, Any]
) -> tuple[dict[str, Any], dict[str, str], dict[str, tuple[int, ...]]]:
    manifest_path = canonical_existing(str(reference / "reference-manifest.json"), "reference manifest")
    require_child(manifest_path, reference, "reference manifest")
    manifest = read_json(manifest_path, 256 * 1024)
    source = profile["authoritative_source"]
    if manifest.get("repo_id") != source["repo_id"] or manifest.get("revision") != source["revision"]:
        raise RecoveryError("Authoritative reference identity does not match the profile")
    files = manifest.get("files")
    if not isinstance(files, dict):
        raise RecoveryError("Reference manifest has no file hash map")
    if files != source["files"]:
        raise RecoveryError("Reference manifest file hashes differ from the pinned profile")
    required = set(profile["required_assets"]) | {source["weight_file"]}
    if set(files) != required:
        missing = sorted(required - set(files))
        extra = sorted(set(files) - required)
        raise RecoveryError(f"Reference asset closure mismatch; missing={missing}, extra={extra}")
    verified: dict[str, str] = {}
    for name in sorted(required):
        candidate = canonical_existing(str(reference / name), f"reference asset {name}")
        require_child(candidate, reference, "reference asset")
        actual = sha256_file(candidate)
        if files[name] != actual:
            raise RecoveryError(f"Reference asset hash mismatch: {name}")
        verified[name] = actual
    if verified[source["weight_file"]] != source["weight_sha256"]:
        raise RecoveryError("Authoritative weight hash differs from the profile")
    config = read_json(reference / "config.json", 256 * 1024)
    validate_config(config, profile["config_contract"])
    inventory = authoritative_inventory(reference / source["weight_file"])
    return config, verified, inventory


def check_cancel(cancel_path: Path) -> None:
    if cancel_path.exists():
        raise Cancelled("Recovery cancelled")


def write_report(path: Path, report: dict[str, Any], maximum: int) -> None:
    encoded = json.dumps(report, sort_keys=True, separators=(",", ":")).encode("utf-8")
    if len(encoded) > maximum:
        raise RecoveryError(f"Machine report exceeds {maximum}-byte profile bound")
    temporary = path.with_suffix(".tmp")
    temporary.write_bytes(encoded)
    os.replace(temporary, path)


def convert(request: dict[str, Any]) -> dict[str, Any]:
    if request.get("schema_version") != 1:
        raise RecoveryError("Unsupported request schema")
    profile_path = canonical_existing(request["profile_path"], "profile")
    profile = read_json(profile_path, 256 * 1024)
    if request.get("profile_id") != profile.get("profile_id"):
        raise RecoveryError("Requested profile identity mismatch")
    if profile.get("architecture") != "llama":
        raise RecoveryError("R2 worker supports only the explicit Llama profile")

    source = canonical_existing(request["source_gguf"], "source GGUF")
    reference = canonical_existing(request["reference_dir"], "reference directory")
    staging_root = canonical_existing(request["staging_root"], "staging root")
    output = Path(request["output_dir"])
    if not output.is_absolute() or output.exists():
        raise RecoveryError("Output must be a new absolute staging directory")
    require_child(output.parent.resolve(strict=True), staging_root, "output")
    cancel_path = Path(request["cancel_path"])
    require_child(cancel_path.parent.resolve(strict=True), staging_root, "cancel sentinel")
    report_path = Path(request["report_path"])
    require_child(report_path.parent.resolve(strict=True), staging_root, "report")

    tier_name = request.get("source_tier")
    tier = profile["gguf_source"]["tiers"].get(tier_name)
    if tier is None:
        raise RecoveryError(f"Source tier {tier_name!r} is not in the profile")
    if source.name != tier["filename"] or source.stat().st_size != tier["size"]:
        raise RecoveryError("GGUF filename or size differs from the pinned tier")
    source_hash = sha256_file(source)
    if source_hash != tier["sha256"]:
        raise RecoveryError("GGUF SHA-256 differs from the pinned tier")
    check_cancel(cancel_path)

    config, reference_hashes, expected = validate_reference(reference, profile)
    if len(expected) != profile["expected_tensor_count"]:
        raise RecoveryError(
            f"Authoritative tensor count {len(expected)} differs from profile count {profile['expected_tensor_count']}"
        )
    estimate = sum(math.prod(shape) * 2 for shape in expected.values())
    max_output = int(request["max_output_bytes"])
    if estimate > max_output:
        raise RecoveryError(f"Estimated FP16 output {estimate} exceeds request bound {max_output}")
    free = shutil.disk_usage(staging_root).free
    if free < estimate + int(request.get("disk_safety_margin_bytes", 0)):
        raise RecoveryError("Insufficient free space for FP16 staging and safety margin")

    output.mkdir(mode=0o700)
    output_weights: dict[str, np.ndarray] = {}
    source_inventory: list[dict[str, Any]] = []
    quant_counts: dict[str, int] = {}
    reader = GGUFReader(str(source), mode="r")
    architecture = metadata_str(reader, "general.architecture")
    if architecture != profile["architecture"]:
        raise RecoveryError(
            f"GGUF architecture {architecture!r} does not match explicit profile {profile['architecture']!r}"
        )
    if len(reader.tensors) > profile["max_tensor_count"]:
        raise RecoveryError("GGUF tensor count exceeds profile bound")
    source_names: set[str] = set()
    allowed = set(tier["allowed_quant_types"])
    for index, tensor in enumerate(reader.tensors):
        check_cancel(cancel_path)
        name = str(tensor.name)
        if name in source_names:
            raise RecoveryError(f"Duplicate source tensor {name}")
        source_names.add(name)
        output_name = map_llama_name(name, int(config["num_hidden_layers"]))
        if output_name in output_weights:
            raise RecoveryError(f"Duplicate mapped output tensor {output_name}")
        quant = qtype_name(int(tensor.tensor_type))
        if quant not in allowed:
            raise RecoveryError(f"Quant type {quant} is not allowed for tier {tier_name}")
        quant_counts[quant] = quant_counts.get(quant, 0) + 1
        array = recover_array(tensor, output_name, config)
        expected_shape = expected.get(output_name)
        if expected_shape is None:
            raise RecoveryError(f"Mapped tensor {output_name} is absent from authoritative closure")
        if tuple(array.shape) != expected_shape:
            raise RecoveryError(
                f"Shape mismatch for {output_name}: {tuple(array.shape)} != {expected_shape}"
            )
        output_weights[output_name] = array
        source_inventory.append(
            {
                "index": index,
                "source_name": name,
                "quant_type": quant,
                "source_shape": [int(value) for value in tensor.shape],
                "output_name": output_name,
                "output_shape": list(expected_shape),
            }
        )

    missing = sorted(set(expected) - set(output_weights))
    extra = sorted(set(output_weights) - set(expected))
    if missing or extra:
        raise RecoveryError(f"Tensor closure mismatch; missing={missing}, extra={extra}")
    if len(output_weights) != profile["expected_tensor_count"]:
        raise RecoveryError("Recovered tensor count differs from profile")
    if quant_counts != tier["quant_inventory"]:
        raise RecoveryError("Source quant inventory differs from the pinned tier")
    tensor_inventory_sha256 = canonical_json_sha256(source_inventory)
    if tensor_inventory_sha256 != tier["tensor_inventory_sha256"]:
        raise RecoveryError("Source tensor inventory differs from the pinned tier")
    check_cancel(cancel_path)

    weight_name = "model.safetensors"
    weight_temp = output / f".{weight_name}.tmp"
    save_file(output_weights, str(weight_temp), metadata={"format": "pt"})
    os.replace(weight_temp, output / weight_name)
    del output_weights
    for asset in profile["required_assets"]:
        check_cancel(cancel_path)
        shutil.copyfile(reference / asset, output / asset)
    # The recovered representation is FP16 even when the authoritative config is BF16.
    output_config = read_json(output / "config.json", 256 * 1024)
    output_config["torch_dtype"] = "float16"
    (output / "config.json").write_text(
        json.dumps(output_config, sort_keys=True, indent=2) + "\n", encoding="utf-8"
    )

    output_hashes = {
        item.name: sha256_file(item)
        for item in sorted(output.iterdir())
        if item.is_file() and not item.is_symlink()
    }
    if set(output_hashes) != set(profile["required_assets"]) | {weight_name}:
        raise RecoveryError("Output asset closure mismatch")
    actual_output_bytes = flat_directory_bytes(output)
    if actual_output_bytes > max_output:
        raise RecoveryError(
            f"Complete output {actual_output_bytes} exceeds request bound {max_output}"
        )

    # Close the path-based TOCTOU window after all reads and copies. A changed source or
    # authoritative asset invalidates the result even if it was valid at initial open.
    if source.stat().st_size != tier["size"] or sha256_file(source) != source_hash:
        raise RecoveryError("GGUF source identity changed during recovery")
    _, final_reference_hashes, final_expected = validate_reference(reference, profile)
    if final_reference_hashes != reference_hashes or final_expected != expected:
        raise RecoveryError("Authoritative reference identity changed during recovery")
    return {
        "schema_version": REPORT_SCHEMA_VERSION,
        "worker_version": WORKER_VERSION,
        "status": "recovered",
        "profile_id": profile["profile_id"],
        "source": {
            "path": str(source),
            "tier": tier_name,
            "size_bytes": source.stat().st_size,
            "sha256": source_hash,
            "architecture": architecture,
            "quant_inventory": quant_counts,
            "tensor_count": len(source_inventory),
            "tensor_inventory_sha256": tensor_inventory_sha256,
        },
        "authoritative_reference": {
            "repo_id": profile["authoritative_source"]["repo_id"],
            "revision": profile["authoritative_source"]["revision"],
            "files": reference_hashes,
            "tensor_count": len(expected),
        },
        "output": {
            "dtype": "float16",
            "tensor_count": len(source_inventory),
            "files": output_hashes,
            "estimated_weight_bytes": estimate,
            "actual_output_bytes": actual_output_bytes,
        },
        "tensor_inventory": source_inventory,
        "skipped_tensors": 0,
        "unknown_tensors": 0,
        "duplicate_tensors": 0,
        "shape_mismatches": 0,
        "non_finite_tensors": 0,
    }


def main() -> int:
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--request", required=True)
    args = parser.parse_args()
    report_path: Path | None = None
    output_path: Path | None = None
    maximum = 1024 * 1024
    try:
        request_path = canonical_existing(args.request, "request")
        request = read_json(request_path, MAX_REQUEST_BYTES)
        report_path = Path(request["report_path"])
        output_path = Path(request["output_dir"])
        profile = read_json(canonical_existing(request["profile_path"], "profile"), 256 * 1024)
        maximum = int(profile["max_report_bytes"])
        report = convert(request)
        write_report(report_path, report, maximum)
        print(json.dumps({"status": "recovered", "report": str(report_path)}))
        return 0
    except Cancelled as exc:
        status = "cancelled"
        code = 130
        error = bounded(exc)
    except Exception as exc:
        status = "failed"
        code = 1
        error = bounded(exc)
        if os.environ.get("LLAMA_MONITOR_GGUF_RECOVERY_TRACE") == "1":
            traceback.print_exc(file=sys.stderr)
    if output_path is not None and output_path.exists():
        shutil.rmtree(output_path, ignore_errors=True)
    failure = {
        "schema_version": REPORT_SCHEMA_VERSION,
        "worker_version": WORKER_VERSION,
        "status": status,
        "error": error,
    }
    if report_path is not None:
        try:
            write_report(report_path, failure, maximum)
        except Exception:
            pass
    print(json.dumps(failure), file=sys.stderr)
    return code


if __name__ == "__main__":
    raise SystemExit(main())
