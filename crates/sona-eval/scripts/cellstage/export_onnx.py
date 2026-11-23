"""Export a trained cell-stage checkpoint (train_cells.py SAVE=…) to the folder sona loads with --cell-model.

    python export_onnx.py CHECKPOINT.pt OUT_DIR

Writes OUT_DIR/cellstage.onnx (input `x` [1, cells, F] raw features, output `logits` [1, cells, NC]; the
feature normalisation is part of the graph) and OUT_DIR/cellstage.json (shapes, candidate order, the
songs it was trained on). The weights derive from Pifuyuu's USTs: keep OUT_DIR local, never commit it.
"""
import json, sys, pathlib
import torch, torch.nn as nn

ck = torch.load(sys.argv[1], weights_only=False)
out = pathlib.Path(sys.argv[2]); out.mkdir(parents=True, exist_ok=True)
F, NC, HID = ck["F"], ck["NC"], ck["HID"]

class M(nn.Module):
    def __init__(s):
        super().__init__()
        s.inp = nn.Sequential(nn.Linear(F, HID), nn.GELU(), nn.Dropout(0.3))
        s.rnn = nn.GRU(HID, HID, num_layers=2, batch_first=True, bidirectional=True, dropout=0.2)
        s.out = nn.Linear(2 * HID, NC)

    def forward(s, x):
        return s.out(s.rnn(s.inp(x))[0])

class Normalised(nn.Module):
    def __init__(s, m, mu, sd):
        super().__init__()
        s.m = m
        s.register_buffer("mu", torch.tensor(mu, dtype=torch.float32))
        s.register_buffer("sd", torch.tensor(sd, dtype=torch.float32))

    def forward(s, x):
        return s.m((x - s.mu) / s.sd)

m = M(); m.load_state_dict(ck["state"]); m.eval()
net = Normalised(m, ck["mu"], ck["sd"]).eval()
x = torch.zeros(1, 37, F)
torch.onnx.export(net, (x,), str(out / "cellstage.onnx"), input_names=["x"], output_names=["logits"],
                  dynamic_axes={"x": {1: "cells"}, "logits": {1: "cells"}}, opset_version=17, dynamo=False)
json.dump({
    "format": 1,
    "features": F,
    "choices": NC,
    "candidates": ["rest", "game_lead", "game_vocals", "sona_grid", "game_lead_octave_up", "game_lead_octave_down"],
    "octave_features": True,
    "trained_on": ck.get("songs", []),
    "epochs": ck.get("EPOCHS"), "hidden": HID, "seed": ck.get("SEED"),
}, open(out / "cellstage.json", "w"), indent=1)
print("wrote", out)
