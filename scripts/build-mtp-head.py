#!/usr/bin/env python3
"""Build a validated, out-of-trunk MTP draft head from a BF16 source.

Why this wrapper exists
-----------------------
Tensor extraction is upstream's job and its details change; we vendor their
script verbatim at ``scripts/vendor/rapid-mlx/extract_mtp_weights.py`` and never
edit it. What upstream gets *wrong for us* is not the math, it is the placement:

1. It writes ``model-mtp.safetensors`` into the quantized MLX model directory.
   ``mlx_lm`` globs ``model*.safetensors`` when loading a trunk, so an in-trunk
   sidecar is picked up as a trunk shard. That sets ``should_shift_norm_weights``
   and applies +1.0 to *every trunk RMSNorm weight*. On an already-converted MLX
   checkpoint the weights are already shifted, so the trunk is double-shifted
   into gibberish — silently, with no error.
2. It rewrites the trunk's ``config.json`` in place to add
   ``mtp_num_hidden_layers``.

So this wrapper runs the extractor, then relocates the sidecar out of the trunk,
restores the trunk config, and refuses to hand back a head that fails a norm
sanity check. That split is deliberate: it keeps working when upstream changes
the extractor, because we only post-process its output.

The norm check is the one that matters. A valid head reads
``pre_fc_norm_embedding`` mean ~= +0.56; a head built by the stale extractor
(which selected norms with ``endswith('norm.weight')`` and so missed the
MTP-specific ``pre_fc_norm_*`` keys) reads ~= -0.44 and gives ~0% draft
acceptance. That single number is the difference between a working draft head and
a dead one, and it is cheap to check, so it is not optional here.

Usage
-----
    python3 scripts/build-mtp-head.py \
        --bf16-source nightmedia/Qwen3.6-27B-Architect-Polaris2-Fable-B-F451-Tess \
        --mlx-model ~/.config/local-llm-foundry/models/mlx/native/nightmedia-27b-mxfp8-mlx

See docs/reference/rapid-mlx-mtp-evidence.md for the requalification procedure
this feeds.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, NoReturn

REPO_ROOT = Path(__file__).resolve().parent.parent
VENDORED_EXTRACTOR = REPO_ROOT / "scripts" / "vendor" / "rapid-mlx" / "extract_mtp_weights.py"
DEFAULT_SIDECAR_ROOT = Path.home() / ".config" / "local-llm-foundry" / "models" / "rapid-mlx" / "mtp-sidecars"

# Upstream's output filename, inside --mlx-model. We move it out under our own
# name; the app's managed sidecar layout uses `mtp.safetensors`.
UPSTREAM_OUTPUT_NAME = "model-mtp.safetensors"
SIDECAR_NAME = "mtp.safetensors"

# A valid head reads ~ +0.56 here; the stale extractor's reads ~ -0.44. We only
# assert the sign, because the magnitude is model-specific and asserting it would
# make this check fail on models it should pass.
NORM_MARKER = "pre_fc_norm"
STALE_EXTRACTOR_MEAN = -0.44

# sha256 of the vendored extractor. Not a security boundary — it exists so a
# re-pull that silently changes the extraction math is visible rather than
# inferred from a bad acceptance rate a day later. See
# scripts/vendor/rapid-mlx/PROVENANCE.md.
VENDORED_EXTRACTOR_SHA256 = (
    "0776ecb720de1b2c6228cd2d6f37abad26ce8c198cde07e1babc7db05616d5a8"
)


def die(message: str) -> NoReturn:
    sys.stderr.write(f"error: {message}\n")
    raise SystemExit(1)


def verify_extractor(extractor: Path) -> None:
    """Refuse an extractor that predates upstream's RMSNorm-shift fix.

    The stale extractor shifted only a hardcoded list of norm suffixes and so
    missed the MTP-specific ``pre_fc_norm_embedding`` / ``pre_fc_norm_hidden``,
    leaving the head's fc-input normalization inverted. Symptom: ~0% draft
    acceptance, no error, backbone fine, tool calls fine — which is exactly why
    it read as a capability limit for weeks rather than a bug.

    The downstream norm check already catches this, but only after a full
    extraction and quantization run. Reading the source first costs nothing and
    names the defect instead of reporting a number. Checked out copies of older
    revisions are still lying around on disk, and ``--extractor`` accepts any
    path, so this is a live footgun rather than a hypothetical one.
    """
    try:
        source = extractor.read_text(encoding="utf-8", errors="replace")
    except OSError as exc:
        die(f"Cannot read extractor {extractor}: {exc}")

    if NORM_MARKER not in source:
        die(
            f"{extractor} does not mention {NORM_MARKER!r}, so it predates upstream's "
            "RMSNorm-shift fix. It will silently build a dead draft head: every trunk "
            "norm shifted correctly, the MTP head's own pre_fc_norm_* left inverted, "
            "~0% draft acceptance and no error anywhere.\nUse the vendored copy "
            f"({VENDORED_EXTRACTOR}) or re-pull a current one from upstream."
        )

    digest = hashlib.sha256(extractor.read_bytes()).hexdigest()
    if digest != VENDORED_EXTRACTOR_SHA256:
        # Not fatal: --extractor is a legitimate override, and a newer upstream
        # copy is the expected reason to use it. Say so rather than deciding.
        sys.stderr.write(
            f"note: extractor sha256 {digest[:16]} does not match the recorded vendored "
            f"copy ({VENDORED_EXTRACTOR_SHA256[:16]}). It passes the norm-fix check, so "
            "this is reported, not refused. If this is a newer upstream revision, update "
            "VENDORED_EXTRACTOR_SHA256 and PROVENANCE.md.\n"
        )


def resolve_interpreter(explicit: str | None) -> str:
    """Find a Python that can import mlx.

    The extractor needs mlx and safetensors. On this machine rapid-mlx is a uv
    tool install, so its interpreter has them and the system python3 does not.
    """
    if explicit:
        return explicit
    rapid_mlx = shutil.which("rapid-mlx")
    if rapid_mlx:
        first_line = Path(rapid_mlx).read_text(errors="replace").splitlines()[0]
        if first_line.startswith("#!"):
            candidate = first_line[2:].strip()
            if Path(candidate).exists():
                return candidate
    return sys.executable


def check_imports(interpreter: str) -> None:
    probe = subprocess.run(
        [interpreter, "-c", "import mlx.core, safetensors"],
        capture_output=True,
        text=True,
    )
    if probe.returncode != 0:
        die(
            f"{interpreter} cannot import mlx.core and safetensors, which the extractor "
            f"needs.\nPass --python pointing at an interpreter that can "
            f"(the rapid-mlx install's own interpreter works).\n{probe.stderr.strip()}"
        )


def slugify(text: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")


def sha256_of(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def norm_means(interpreter: str, sidecar: Path) -> dict[str, float]:
    """Read the pre_fc_norm_* means out of the built sidecar.

    Done in a subprocess with the mlx-capable interpreter so this wrapper itself
    stays runnable under a bare system python3.
    """
    snippet = (
        "import json,sys;import mlx.core as mx;"
        "w=mx.load(sys.argv[1]);"
        f"print(json.dumps({{k:float(v.mean()) for k,v in w.items() if '{NORM_MARKER}' in k}}))"
    )
    probe = subprocess.run(
        [interpreter, "-c", snippet, str(sidecar)],
        capture_output=True,
        text=True,
    )
    if probe.returncode != 0:
        die(f"Could not read norms back from {sidecar}:\n{probe.stderr.strip()}")
    return json.loads(probe.stdout)


def validate_norms(means: dict[str, float]) -> dict[str, Any]:
    """Refuse a head whose fc-input normalization is inverted.

    Absence of the markers is reported, not treated as a pass: a future
    architecture may not have them, and that is a reason to look, not to ship.
    """
    if not means:
        die(
            f"No '{NORM_MARKER}*' tensors found in the built head, so the norm-shift "
            "check could not run. Refusing to certify a head that cannot be "
            "validated. Inspect the sidecar's tensor names before using it; if this "
            "architecture genuinely lacks these tensors, that fact belongs in "
            "docs/reference/rapid-mlx-mtp-evidence.md first."
        )
    inverted = {name: mean for name, mean in means.items() if mean <= 0}
    if inverted:
        detail = ", ".join(f"{name}={mean:+.4f}" for name, mean in sorted(inverted.items()))
        die(
            f"Norm shift is missing or inverted: {detail}.\nA valid head reads a "
            f"positive mean (~+0.56); the stale extractor read ~{STALE_EXTRACTOR_MEAN}. "
            "This head would give ~0% draft acceptance, which is exactly the failure "
            "that produced the void receipts. Re-pull the extractor from upstream main "
            "and rebuild; do not use this file."
        )
    return {
        "pre_fc_norm_means": means,
        "all_positive": True,
        "expected_mean_stale_extractor": STALE_EXTRACTOR_MEAN,
        "method": "mean of every pre_fc_norm_* tensor in the built sidecar",
    }


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Build a validated, out-of-trunk MTP draft head.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--bf16-source",
        required=True,
        help="HF repo id or local path of the BF16 source that carries the mtp.* tensors.",
    )
    parser.add_argument(
        "--mlx-model",
        required=True,
        help="Quantized MLX trunk directory. Supplies the quantization config; is NOT modified.",
    )
    parser.add_argument(
        "--revision",
        default=None,
        help="Immutable Hugging Face commit/revision for the BF16 source.",
    )
    parser.add_argument(
        "--out",
        default=None,
        help=f"Sidecar output directory. Default: {DEFAULT_SIDECAR_ROOT}/<trunk-slug>/",
    )
    parser.add_argument("--bits", type=int, default=None, help="Override quantization bits.")
    parser.add_argument("--group-size", type=int, default=None, help="Override group size.")
    parser.add_argument(
        "--python",
        default=None,
        help="Interpreter that can import mlx. Default: the one rapid-mlx runs under.",
    )
    parser.add_argument(
        "--extractor",
        default=str(VENDORED_EXTRACTOR),
        help="Extractor script. Default: the vendored upstream copy.",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Overwrite an existing sidecar at --out.",
    )
    args = parser.parse_args()

    trunk = Path(args.mlx_model).expanduser().resolve()
    if not trunk.is_dir():
        die(f"--mlx-model is not a directory: {trunk}")
    extractor = Path(args.extractor).expanduser().resolve()
    if not extractor.is_file():
        die(f"Extractor not found: {extractor}")
    verify_extractor(extractor)

    out_dir = (
        Path(args.out).expanduser().resolve()
        if args.out
        else DEFAULT_SIDECAR_ROOT / slugify(trunk.name)
    )
    # The whole point of this wrapper. An in-trunk sidecar double-shifts the
    # trunk's norms on the next load, so refusing here is not pedantry.
    if out_dir == trunk or trunk in out_dir.parents:
        die(
            f"--out would place the sidecar inside the trunk ({out_dir}).\nmlx_lm globs "
            "model*.safetensors when loading a trunk, so an in-trunk sidecar is read as "
            "a trunk shard and double-shifts every trunk RMSNorm weight. Choose a "
            "directory outside the model."
        )

    sidecar_path = out_dir / SIDECAR_NAME
    if sidecar_path.exists() and not args.force:
        die(f"{sidecar_path} already exists. Pass --force to rebuild it.")

    interpreter = resolve_interpreter(args.python)
    check_imports(interpreter)

    staged = trunk / UPSTREAM_OUTPUT_NAME
    if staged.exists():
        die(
            f"{staged} already exists inside the trunk. Move or quarantine it before "
            "building, so this run cannot be confused with a previous one — and so the "
            "trunk is not left loadable-but-double-shifted."
        )

    # Upstream mutates the trunk config.json; snapshot it so we can put it back.
    config_path = trunk / "config.json"
    config_before = config_path.read_bytes() if config_path.exists() else None

    command = [
        interpreter,
        str(extractor),
        "--hf-model",
        args.bf16_source,
        "--mlx-model",
        str(trunk),
    ]
    if args.revision:
        command += ["--revision", args.revision]
    if args.bits is not None:
        command += ["--bits", str(args.bits)]
    if args.group_size is not None:
        command += ["--group-size", str(args.group_size)]

    sys.stderr.write(f"Extracting with {extractor.name} (vendored upstream copy)\n")
    sys.stderr.write(f"  interpreter: {interpreter}\n")
    result = subprocess.run(command)

    try:
        if result.returncode != 0:
            die(f"Extractor exited {result.returncode}; nothing was written to {out_dir}.")
        if not staged.exists():
            die(
                f"Extractor reported success but {staged} does not exist. Upstream may "
                "have changed its output path; check the extractor and update "
                "UPSTREAM_OUTPUT_NAME in this wrapper."
            )

        validation = validate_norms(norm_means(interpreter, staged))

        out_dir.mkdir(parents=True, exist_ok=True)
        shutil.move(str(staged), str(sidecar_path))
    finally:
        # Restore the trunk even on failure: a half-built run must not leave the
        # trunk carrying an mtp_num_hidden_layers key or a stray sidecar.
        if config_before is not None and config_path.exists():
            if config_path.read_bytes() != config_before:
                config_path.write_bytes(config_before)
                sys.stderr.write(
                    "Restored the trunk's config.json; the extractor had modified it "
                    "in place.\n"
                )
        if staged.exists():
            quarantine = trunk.parent / ".quarantine-in-trunk-sidecars"
            quarantine.mkdir(parents=True, exist_ok=True)
            shutil.move(str(staged), str(quarantine / f"{trunk.name}__{UPSTREAM_OUTPUT_NAME}"))
            sys.stderr.write(f"Moved a leftover in-trunk sidecar to {quarantine}.\n")

    provenance = {
        "schema_version": 1,
        "kind": "mtp_sidecar",
        "status": "built_unvalidated_online",
        "note": (
            "Built by scripts/build-mtp-head.py. The norm check below is an offline "
            "sanity check, not a qualification: served acceptance is only established "
            "by scripts/rapid-mlx-requalify-spec-decode.mjs."
        ),
        "file": SIDECAR_NAME,
        "sha256": sha256_of(sidecar_path),
        "source": {
            "bf16_source": args.bf16_source,
            "revision": args.revision,
            "trunk": str(trunk),
            "extracted_with": f"vendored {extractor.name}",
            "extractor_sha256": sha256_of(extractor),
        },
        "validation": validation,
        "known_good_positive_control": "mlx-community/Qwen3.6-27B-MTP-4bit",
        "built_at": datetime.now(timezone.utc).isoformat(),
    }
    (out_dir / "provenance.json").write_text(f"{json.dumps(provenance, indent=2)}\n")

    means_map: dict[str, float] = validation["pre_fc_norm_means"]
    means = ", ".join(f"{name.split('.')[-2]}={mean:+.4f}" for name, mean in sorted(means_map.items()))
    sys.stderr.write(f"\nSidecar: {sidecar_path}\n")
    sys.stderr.write(f"Norm check passed ({means})\n")
    sys.stderr.write(f"Provenance: {out_dir / 'provenance.json'}\n")
    sys.stderr.write(
        "\nThis head is not qualified yet. Measure it served:\n"
        f"  node scripts/rapid-mlx-requalify-spec-decode.mjs \\\n"
        f"    --model {trunk} \\\n"
        f"    --speculative-control-model mlx-community/Qwen3.6-27B-MTP-4bit \\\n"
        f"    --speculative-model {out_dir} \\\n"
        f"    --profile-alias <hf-alias-for-this-family> \\\n"
        f"    --out tmp/requalify-$(date +%Y%m%d)\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
