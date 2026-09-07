"""Regenerate `resources/silero_vad_16k.onnx` from upstream Silero VAD.

The published model picks between an 8 kHz and a 16 kHz sub-model with an ONNX
`If`, and nests further `If`s for shape handling. `tract` (our pure-Rust runtime,
chosen so silence detection builds on every target including Intel Mac) cannot
analyse `If` at all, so the published file fails to load. This splices out the
16 kHz branch and constant-folds the rest away, leaving a graph of plain ops.

The result is verified bit-identical to the original before it is written.

    pip install onnx onnxruntime numpy
    python scripts/build-silero-vad.py
"""

import hashlib
import pathlib
import subprocess
import sys
import tempfile

import numpy as np
import onnx
import onnxruntime as ort
from onnx import TensorProto, helper

# Pinned so a regeneration is reproducible; bump deliberately, then re-verify.
UPSTREAM = "https://github.com/snakers4/silero-vad/raw/v5.1.2/src/silero_vad/data/silero_vad.onnx"
DEST = pathlib.Path(__file__).resolve().parent.parent / "apps/desktop/src-tauri/resources/silero_vad_16k.onnx"
CHUNK, STATE = 512, (2, 1, 128)


def fetch(tmp: pathlib.Path) -> pathlib.Path:
    src = tmp / "silero_upstream.onnx"
    subprocess.run(["curl", "-sL", "-o", str(src), UPSTREAM], check=True)
    print(f"upstream {UPSTREAM}\n  sha256 {hashlib.sha256(src.read_bytes()).hexdigest()}")
    return src


def splice_16k(src: pathlib.Path, tmp: pathlib.Path) -> pathlib.Path:
    model = onnx.load(src)
    top = model.graph
    branch = next(a.g for a in top.node[2].attribute if a.name == "then_branch")
    out0, out1 = (o.name for o in branch.output)
    nodes = list(branch.node)
    nodes.append(helper.make_node("Identity", [out0], ["output"]))
    nodes.append(helper.make_node("Identity", [out1], ["stateN"]))
    graph = helper.make_graph(
        nodes,
        "silero_16k",
        [
            helper.make_tensor_value_info("input", TensorProto.FLOAT, [1, CHUNK]),
            helper.make_tensor_value_info("state", TensorProto.FLOAT, list(STATE)),
        ],
        [
            helper.make_tensor_value_info("output", TensorProto.FLOAT, [1, 1]),
            helper.make_tensor_value_info("stateN", TensorProto.FLOAT, list(STATE)),
        ],
        list(branch.initializer) + list(top.initializer),
    )
    spliced = helper.make_model(graph, opset_imports=list(model.opset_import))
    spliced.ir_version = model.ir_version
    onnx.checker.check_model(spliced)
    path = tmp / "spliced.onnx"
    onnx.save(spliced, path)
    return path


def fold(spliced: pathlib.Path, tmp: pathlib.Path) -> pathlib.Path:
    out = tmp / "folded.onnx"
    opts = ort.SessionOptions()
    # BASIC only: EXTENDED introduces com.microsoft ops (FusedConv) that tract cannot read.
    opts.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_BASIC
    opts.optimized_model_filepath = str(out)
    ort.InferenceSession(str(spliced), opts, providers=["CPUExecutionProvider"])
    folded = onnx.load(out)
    left = [n.op_type for n in folded.graph.node if n.op_type == "If"]
    if left:
        sys.exit(f"error: {len(left)} If node(s) survived; tract cannot load this")
    foreign = {n.domain for n in folded.graph.node} - {""}
    if foreign:
        sys.exit(f"error: non-standard op domains {foreign}; tract cannot load this")
    return out


def verify(original: pathlib.Path, folded: pathlib.Path) -> None:
    a = ort.InferenceSession(str(original), providers=["CPUExecutionProvider"])
    b = ort.InferenceSession(str(folded), providers=["CPUExecutionProvider"])
    rng = np.random.default_rng(7)
    t = np.arange(CHUNK, dtype=np.float32) / 16000.0
    windows = [np.zeros((1, CHUNK), np.float32)]
    windows += [(0.3 * np.sin(2 * np.pi * f * t)).astype(np.float32).reshape(1, CHUNK) for f in (120.0, 440.0, 1200.0)]
    windows += [rng.standard_normal((1, CHUNK)).astype(np.float32) * 0.1 for _ in range(60)]

    sa = np.zeros(STATE, np.float32)
    sb = np.zeros(STATE, np.float32)
    worst = 0.0
    for w in windows:
        ra = a.run(None, {"input": w, "state": sa, "sr": np.array(16000, np.int64)})
        rb = b.run(None, {"input": w, "state": sb})
        worst = max(worst, abs(float(ra[0][0][0]) - float(rb[0][0][0])), float(np.max(np.abs(ra[1] - rb[1]))))
        sa, sb = ra[1], rb[1]
    if worst != 0.0:
        sys.exit(f"error: folded model diverges from upstream by {worst:.3e}")
    print(f"verified bit-identical over {len(windows)} windows with state carried forward")


def main() -> None:
    with tempfile.TemporaryDirectory() as raw:
        tmp = pathlib.Path(raw)
        src = fetch(tmp)
        folded = fold(splice_16k(src, tmp), tmp)
        verify(src, folded)
        data = folded.read_bytes()
    DEST.write_bytes(data)
    print(f"wrote {DEST} ({len(data):,} bytes)\n  sha256 {hashlib.sha256(data).hexdigest()}")


if __name__ == "__main__":
    main()
