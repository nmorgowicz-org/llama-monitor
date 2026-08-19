#!/usr/bin/env python3
"""Compare a pinned R3 quantized model with its exact recovered FP16 source."""

import argparse
import json
import time

import mlx.core as mx
import numpy as np
from mlx.utils import tree_flatten
from mlx_lm import load
from mlx_lm.utils import dequantize_model
from safetensors import safe_open


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", required=True)
    parser.add_argument("--quantized", required=True)
    parser.add_argument("--recipe", required=True)
    parser.add_argument("--limit", type=int, default=8)
    args = parser.parse_args()
    started = time.monotonic()
    model, _ = load(args.quantized, lazy=True)
    load_seconds = time.monotonic() - started
    model = dequantize_model(model)
    parameters = dict(tree_flatten(model.parameters()))
    rows = []
    with safe_open(args.source, framework="np") as source:
        source_keys = set(source.keys())
        parameter_keys = set(parameters)
        if source_keys != parameter_keys:
            raise SystemExit(
                json.dumps(
                    {
                        "error": "parameter key closure mismatch",
                        "missing": sorted(source_keys - parameter_keys)[:32],
                        "extra": sorted(parameter_keys - source_keys)[:32],
                    }
                )
            )
        for key in sorted(source_keys):
            expected = source.get_tensor(key).astype(np.float32)
            value = parameters.pop(key)
            mx.eval(value)
            actual = np.asarray(value.astype(mx.float32))
            if expected.shape != actual.shape:
                raise SystemExit(f"shape mismatch: {key}")
            if not np.isfinite(expected).all() or not np.isfinite(actual).all():
                raise SystemExit(f"non-finite tensor: {key}")
            delta = np.abs(expected - actual)
            denominator = float(
                np.linalg.norm(expected.ravel()) * np.linalg.norm(actual.ravel())
            )
            cosine = (
                float(np.dot(expected.ravel(), actual.ravel()) / denominator)
                if denominator
                else 1.0
            )
            if not np.isfinite(delta).all() or not np.isfinite(cosine):
                raise SystemExit(f"non-finite fidelity metric: {key}")
            rows.append(
                {
                    "name": key,
                    "max_abs": float(delta.max(initial=0)),
                    "mean_abs": float(delta.mean()),
                    "cosine": cosine,
                }
            )
            del expected, actual, value
    if not rows:
        raise SystemExit("fidelity comparison produced no tensors")
    rows.sort(key=lambda row: row["mean_abs"], reverse=True)
    print(
        json.dumps(
            {
                "schema_version": 1,
                "recipe": args.recipe,
                "source_tensor_count": len(rows),
                "load_seconds": load_seconds,
                "validation_seconds": time.monotonic() - started,
                "global_max_abs": max(row["max_abs"] for row in rows),
                "global_mean_abs": float(np.mean([row["mean_abs"] for row in rows])),
                "global_min_cosine": min(row["cosine"] for row in rows),
                "worst": rows[: max(1, min(args.limit, 32))],
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
