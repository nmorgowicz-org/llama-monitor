#!/usr/bin/env python3
"""Bounded tensor-parity evidence for the pinned R2 corpus."""

import argparse
import json
import math
from pathlib import Path

import numpy as np
from safetensors import safe_open


def _load_reference_without_framework(path: str) -> dict[str, np.ndarray]:
    """Read the small dtype subset used by the pinned authoritative fixture.

    safetensors' NumPy adapter cannot currently materialize BF16. Reading its
    documented on-disk representation directly keeps this evidence tool
    independent of MLX/Metal and avoids silently changing the validation host.
    """
    raw = Path(path).read_bytes()
    if len(raw) < 8:
        raise SystemExit("reference safetensors header is truncated")
    header_length = int.from_bytes(raw[:8], "little")
    data_start = 8 + header_length
    if data_start > len(raw):
        raise SystemExit("reference safetensors header length exceeds file size")
    header = json.loads(raw[8:data_start])
    tensors = {}
    for key, metadata in header.items():
        if key == "__metadata__":
            continue
        shape = tuple(metadata["shape"])
        start, end = metadata["data_offsets"]
        payload = memoryview(raw)[data_start + start : data_start + end]
        dtype = metadata["dtype"]
        if dtype == "BF16":
            words = np.frombuffer(payload, dtype="<u2")
            value = (words.astype(np.uint32) << 16).view(np.float32)
        elif dtype == "F16":
            value = np.frombuffer(payload, dtype="<f2").astype(np.float32)
        elif dtype == "F32":
            value = np.frombuffer(payload, dtype="<f4").astype(np.float32)
        else:
            raise SystemExit(f"unsupported reference dtype: {dtype} ({key})")
        if value.size != math.prod(shape):
            raise SystemExit(f"reference tensor byte-size mismatch: {key}")
        tensors[key] = value.reshape(shape)
    return tensors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--reference", required=True)
    parser.add_argument("--recovered", required=True)
    parser.add_argument("--limit", type=int, default=12)
    args = parser.parse_args()
    rows = []
    try:
        with safe_open(args.reference, framework="np") as reference:
            reference_tensors = {
                key: reference.get_tensor(key).astype(np.float32) for key in reference.keys()
            }
    except TypeError as error:
        if "bfloat16" not in str(error):
            raise
        reference_tensors = _load_reference_without_framework(args.reference)
    with safe_open(args.recovered, framework="np") as recovered:
        if set(reference_tensors) != set(recovered.keys()):
            raise SystemExit("tensor key closure mismatch")
        for key, expected in reference_tensors.items():
            actual = recovered.get_tensor(key).astype(np.float32)
            if expected.shape != actual.shape:
                raise SystemExit(f"shape mismatch: {key}")
            delta = np.abs(expected - actual)
            denom = float(np.linalg.norm(expected.ravel()) * np.linalg.norm(actual.ravel()))
            cosine = float(np.dot(expected.ravel(), actual.ravel()) / denom) if denom else 1.0
            rows.append(
                {
                    "name": key,
                    "max_abs": float(delta.max(initial=0)),
                    "mean_abs": float(delta.mean()),
                    "cosine": cosine,
                }
            )
    rows.sort(key=lambda row: row["mean_abs"], reverse=True)
    print(
        json.dumps(
            {
                "tensor_count": len(rows),
                "worst": rows[: max(1, min(args.limit, 64))],
                "global_max_abs": max(row["max_abs"] for row in rows),
                "global_min_cosine": min(row["cosine"] for row in rows),
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
