#!/usr/bin/env python3
"""Inspect and repair a Rapid-MLX MTP sidecar.

The command is intentionally local-source only.  The caller is responsible for
downloading and pairing immutable parents; this tool refuses to infer them.
It reads safetensors headers before loading tensors, accepts both ``mtp.`` and
bare namespaces, and writes only an external ``mtp.safetensors`` sidecar.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

REQUIRED = (
    "fc.weight",
    "pre_fc_norm_embedding.weight",
    "pre_fc_norm_hidden.weight",
)
TRUNK_TYPES = {"qwen3_5", "qwen3_5_moe", "hy_v3"}
HEAD_TYPES = {"qwen3_5_mtp", "head"}


class RepairError(RuntimeError):
    pass


def config(root: Path) -> dict[str, Any]:
    try:
        value = json.loads((root / "config.json").read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise RepairError(f"cannot read {root / 'config.json'}: {exc}") from exc
    if not isinstance(value, dict):
        raise RepairError(f"{root / 'config.json'} is not a JSON object")
    return value


def config_value(value: dict[str, Any], key: str) -> Any:
    if key in value:
        return value[key]
    nested = value.get("text_config")
    return nested.get(key) if isinstance(nested, dict) else None


def tensor_files(root: Path) -> dict[str, Path]:
    index = root / "model.safetensors.index.json"
    if index.is_file():
        try:
            weight_map = json.loads(index.read_text(encoding="utf-8"))["weight_map"]
        except (OSError, KeyError, TypeError, json.JSONDecodeError) as exc:
            raise RepairError(f"invalid safetensors index {index}: {exc}") from exc
        return {key: root / name for key, name in weight_map.items()}
    files = sorted(root.glob("*.safetensors"))
    if not files:
        raise RepairError(f"no safetensors files found in {root}")
    return {key: path for path in files for key in header_keys(path)}


def header_keys(path: Path) -> list[str]:
    try:
        with path.open("rb") as handle:
            size = int.from_bytes(handle.read(8), "little")
            if size <= 0 or size > 64 * 1024 * 1024:
                raise RepairError(f"implausible safetensors header in {path}")
            header = json.loads(handle.read(size))
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        raise RepairError(f"invalid safetensors header {path}: {exc}") from exc
    return [key for key in header if key != "__metadata__"]


def canonical(key: str) -> str | None:
    if key.startswith("mtp."):
        return key[4:]
    if key == "fc.weight" or key.startswith("layers.") or key.startswith("pre_fc_norm"):
        return key
    return None


def inspect(root: Path) -> dict[str, Any]:
    root = root.expanduser().resolve()
    if not root.is_dir():
        raise RepairError(f"snapshot is not a directory: {root}")
    cfg = config(root)
    files = tensor_files(root)
    mtp = {key: path for key, path in files.items() if canonical(key) is not None}
    names = sorted({canonical(key) for key in mtp if canonical(key) is not None})
    missing = sorted(set(REQUIRED) - set(names))
    model_type = str(config_value(cfg, "model_type") or "")
    if model_type in HEAD_TYPES:
        kind = "head_only"
    elif not names:
        kind = "none"
    elif missing:
        kind = "partial"
    elif any(canonical(key) is None for key in files):
        kind = "fused"
    else:
        kind = "complete"
    return {
        "snapshot": str(root),
        "model_type": model_type,
        "kind": kind,
        "mtp_keys": names,
        "missing_required": missing,
        "mtp_num_hidden_layers": config_value(cfg, "mtp_num_hidden_layers"),
        "namespace": "mtp." if any(key.startswith("mtp.") for key in mtp) else "",
    }


def load_mlx() -> Any:
    try:
        import mlx.core as mx
    except ImportError as exc:
        raise RepairError("MLX is required; run this command with the Rapid-MLX Python environment") from exc
    return mx


def load_weights(root: Path, info: dict[str, Any]) -> dict[str, Any]:
    mx = load_mlx()
    try:
        from safetensors import safe_open
    except ImportError as exc:
        raise RepairError("safetensors is required; run this command with the Rapid-MLX Python environment") from exc
    files = tensor_files(root)
    selected: dict[Path, set[str]] = {}
    for key in info["mtp_keys"]:
        original = next((name for name in files if canonical(name) == key), None)
        if original is None:
            raise RepairError(f"MTP tensor {key!r} disappeared from {root}")
        selected.setdefault(files[original], set()).add(original)
    result: dict[str, Any] = {}
    for path, names in selected.items():
        # Read only selected tensors.  A fused MTP head can share a multi-GB
        # trunk shard; loading that entire shard defeats header-first hygiene.
        with safe_open(str(path), framework="numpy") as handle:
            for original in names:
                key = canonical(original)
                if key is not None:
                    result[key] = mx.array(handle.get_tensor(original))
    return result


def validate_weights(weights: dict[str, Any]) -> dict[str, Any]:
    missing = sorted(set(REQUIRED) - set(weights))
    if missing:
        raise RepairError(f"repaired sidecar is missing required tensors: {', '.join(missing)}")
    means: dict[str, float] = {}
    for key, value in weights.items():
        if "pre_fc_norm" in key:
            means[key] = float(value.mean().item())
    if not means or any(not math.isfinite(value) or value <= 0 for value in means.values()):
        raise RepairError(f"pre_fc_norm sign check failed: {means}")
    return {"pre_fc_norm_means": means, "all_positive": True}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def recipe_value(node: dict[str, Any], name: str, stores: dict[Path, dict[str, Any]], mx: Any) -> Any:
    if str(node.get("merge_method", "nuslerp")).lower() != "nuslerp":
        raise RepairError("only mergekit-style NuSLERP recipes are supported")
    models = node.get("models")
    if not isinstance(models, list) or len(models) != 2:
        raise RepairError("NuSLERP nodes must contain exactly two models")
    values = []
    for child in models:
        if not isinstance(child, dict):
            raise RepairError("invalid recipe child")
        if "dir" in child:
            source = Path(str(child["dir"])).expanduser().resolve()
            if not source.is_dir() or source not in stores:
                raise RepairError(f"recipe parent is unavailable: {source}")
            try:
                values.append(stores[source][name].astype(mx.float32))
            except KeyError as exc:
                raise RepairError(f"recipe parent {source} lacks MTP tensor {name!r}") from exc
        else:
            values.append(recipe_value(child, name, stores, mx).astype(mx.float32))
    total = float(models[0].get("weight", 1.0)) + float(models[1].get("weight", 1.0))
    t = 0.5 if abs(total) < 1e-6 else float(models[1].get("weight", 1.0)) / total
    shape = values[0].shape
    v0 = values[0].reshape(-1)
    v1 = values[1].reshape(-1)
    n0 = mx.maximum(mx.linalg.norm(v0), 1e-7)
    n1 = mx.maximum(mx.linalg.norm(v1), 1e-7)
    cosine = mx.clip(mx.sum((v0 / n0) * (v1 / n1)), -1.0, 1.0)
    theta = mx.arccos(cosine)
    sine = mx.sin(theta)
    if abs(float(sine.item())) < 1e-8:
        result = (1.0 - t) * v0 + t * v1
    else:
        result = (mx.sin((1.0 - t) * theta) / sine) * v0 + (mx.sin(t * theta) / sine) * v1
    return result.reshape(shape)


def recipe_parents(node: dict[str, Any], output: list[dict[str, Any]], location: str = "recipe") -> None:
    if str(node.get("merge_method", "nuslerp")).lower() != "nuslerp":
        raise RepairError(f"{location} uses an unsupported merge method")
    models = node.get("models")
    if not isinstance(models, list) or len(models) != 2:
        raise RepairError(f"{location} must contain exactly two models")
    for index, child in enumerate(models):
        child_location = f"{location}.models[{index}]"
        if not isinstance(child, dict):
            raise RepairError(f"{child_location} is not an object")
        if "dir" in child:
            path = Path(str(child["dir"])).expanduser().resolve()
            if not path.is_dir():
                raise RepairError(f"recipe parent is unavailable: {path}")
            revision = child.get("revision")
            if child.get("repo_id") and (not isinstance(revision, str) or len(revision) < 7):
                raise RepairError(f"{child_location} requires an immutable revision")
            output.append({"tree_path": child_location, "dir": str(path), "repo_id": child.get("repo_id"), "revision": revision, "weight": float(child.get("weight", 1.0))})
        else:
            recipe_parents(child, output, child_location)


def repair(args: argparse.Namespace) -> dict[str, Any]:
    target_root = Path(args.target).expanduser().resolve()
    target = inspect(target_root)
    if target["kind"] not in {"none", "complete", "partial", "fused"}:
        raise RepairError(f"target does not contain a usable MTP namespace: {target['kind']}")
    mx = load_mlx()
    target_weights = {} if target["kind"] == "none" else load_weights(target_root, target)
    stores: dict[Path, dict[str, Any]] = {}
    recipe_meta = None
    source_info = None
    if args.recipe:
        recipe_path = Path(args.recipe).expanduser().resolve()
        recipe = json.loads(recipe_path.read_text(encoding="utf-8"))
        parents: list[dict[str, Any]] = []
        recipe_parents(recipe, parents)
        for parent in parents:
            parent_path = Path(parent["dir"])
            info = inspect(parent_path)
            if info["kind"] not in {"complete", "fused"}:
                raise RepairError(f"recipe parent is not a complete MTP source: {parent_path}")
            check_compatibility(target_root, parent_path)
            stores[parent_path] = load_weights(parent_path, info)
        check_weight_shapes(stores.values())
        merged = {key: recipe_value(recipe, key, stores, mx).astype(mx.bfloat16) for key in REQUIRED}
        # Preserve additional MTP tensors from the target, while replacing the
        # recipe-covered tensors so the head follows the merge exactly.
        merged.update({key: value for key, value in target_weights.items() if key not in merged})
        recipe_meta = {"digest": sha256(recipe_path), "method": "nuslerp", "compute_dtype": "bfloat16", "parents": parents}
        mode = "recipe_reconstruction"
    else:
        if not args.source:
            raise RepairError("one of --source or --recipe is required")
        source_root = Path(args.source).expanduser().resolve()
        source = inspect(source_root)
        if source["kind"] not in {"complete", "fused"}:
            raise RepairError(f"source is not a complete MTP head: {source['kind']}")
        check_compatibility(target_root, source_root)
        source_weights = load_weights(source_root, source)
        if args.source_format == "hf":
            source_weights = {key: (value + 1.0 if "norm" in key and key.endswith(".weight") else value) for key, value in source_weights.items()}
        merged = dict(target_weights)
        merged.update({key: value for key, value in source_weights.items() if key not in merged})
        source_info = source
        mode = "direct_parent"
    validation = validate_weights(merged)
    output = Path(args.output).expanduser().resolve()
    if output == target_root or target_root in output.parents:
        raise RepairError("refusing to write a sidecar inside the target trunk")
    output.parent.mkdir(parents=True, exist_ok=True)
    prefix = target["namespace"] or (source_info or {}).get("namespace", "")
    mx.save_safetensors(str(output), {f"{prefix}{key}": value for key, value in merged.items()})
    provenance = {
        "schema_version": 2,
        "status": "candidate",
        "repair_mode": mode,
        "target": target,
        "source": source_info,
        "recipe": recipe_meta,
        "validation": validation,
        "output": str(output),
        "sha256": sha256(output),
        "created_at": datetime.now(timezone.utc).isoformat(),
    }
    output.with_name("provenance.json").write_text(json.dumps(provenance, indent=2) + "\n", encoding="utf-8")
    return provenance


def check_compatibility(target_root: Path, source_root: Path) -> None:
    target = config(target_root)
    source = config(source_root)
    for key in ("hidden_size", "num_attention_heads", "num_key_value_heads", "num_hidden_layers"):
        target_value = config_value(target, key)
        source_value = config_value(source, key)
        if target_value is not None and source_value is not None and target_value != source_value:
            raise RepairError(f"parent architecture mismatch for {key}: target={target_value}, source={source_value}")


def check_weight_shapes(stores: Any) -> None:
    expected: dict[str, tuple[int, ...]] = {}
    for store in stores:
        for key, value in store.items():
            shape = tuple(int(dimension) for dimension in value.shape)
            if key in expected and expected[key] != shape:
                raise RepairError(f"recipe parent shape mismatch for {key}: {expected[key]} vs {shape}")
            expected[key] = shape


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    inspect_parser = sub.add_parser("inspect")
    inspect_parser.add_argument("snapshot")
    repair_parser = sub.add_parser("repair")
    repair_parser.add_argument("--target", required=True)
    repair_parser.add_argument("--source")
    repair_parser.add_argument("--source-format", choices=("mlx", "hf"), default="mlx")
    repair_parser.add_argument("--recipe")
    repair_parser.add_argument("--output", required=True)
    try:
        args = parser.parse_args(argv)
        result = inspect(args.snapshot) if args.command == "inspect" else repair(args)
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0
    except (RepairError, OSError, ValueError, json.JSONDecodeError) as exc:
        print(json.dumps({"status": "failed", "error": str(exc)}), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
