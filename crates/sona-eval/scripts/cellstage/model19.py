"""The cell stage's network and its training (the prototype's `train_cells.py`, ADR-0017), with the loss
options of utau-edit-cost 19. Defaults reproduce the prototype's recipe: CE on the first candidate with the
UST's pitch, HID 96, 15 epochs, AdamW 2e-3, windows of 256 cells, batch 16, seed 0."""
import os
import numpy as np
import torch
import torch.nn as nn

torch.set_num_threads(int(os.environ.get("THREADS", "8")))
NC = 6


class M(nn.Module):
    def __init__(s, F, HID):
        super().__init__()
        s.inp = nn.Sequential(nn.Linear(F, HID), nn.GELU(), nn.Dropout(0.3))
        s.rnn = nn.GRU(HID, HID, num_layers=2, batch_first=True, bidirectional=True, dropout=0.2)
        s.out = nn.Linear(2 * HID, NC)

    def forward(s, x):
        return s.out(s.rnn(s.inp(x))[0])


def windows(n, W=256, step=128):
    return [(a, min(n, a + W)) for a in range(0, max(1, n - W // 2), step)]


def edge_weights(d, w_edge):
    """Edit-cost weights per cell: cells next to a UST note edge (either side) weigh `w_edge`, others 1. A cell
    wrong at an edge moves a start or end (4 each, up to 12 at a legato boundary); inside a note a wrong
    cell is mostly a pitch blip."""
    note, ref = d["note"], d["ref"]
    n = len(note)
    w = np.ones(n, np.float32)
    if w_edge == 1:
        return w
    edge = np.zeros(n + 1, bool)
    edge[1:-1] = (note[1:] != note[:-1]) | (ref[1:] != ref[:-1])
    near = edge[:-1] | edge[1:]
    w[near] = w_edge
    return w


def train(tr, cfg):
    """tr: songs (lib19.load_song dicts). cfg: width, hid, epochs, seed, loss ('first' | 'set'), w_edge,
    w_pair (pairwise same-note term)."""
    seed = cfg.get("seed", 0)
    torch.manual_seed(seed)
    np.random.seed(seed)
    F = cfg["width"]
    Xs = [d["X"][:, :F] for d in tr]
    span = [d["span"] for d in tr]
    Xall = np.concatenate([x[s] for x, s in zip(Xs, span)])
    mu, sd = Xall.mean(0), Xall.std(0) + 1e-5
    m = M(F, cfg.get("hid", 96))
    opt = torch.optim.AdamW(m.parameters(), lr=2e-3, weight_decay=1e-2)
    rows = []
    for d, x, s in zip(tr, Xs, span):
        lo, hi = np.nonzero(s)[0][[0, -1]]
        hi += 1
        xs = ((x[lo:hi] - mu) / sd).astype(np.float32)
        y = d["y"][lo:hi]
        G = d["G"][lo:hi] if cfg.get("loss") == "set" else None
        w = edge_weights(d, cfg.get("w_edge", 1))[lo:hi]
        same = np.zeros(hi - lo, np.float32)
        nt = d["note"][lo:hi]
        same[1:] = ((nt[1:] == nt[:-1]) & (nt[1:] >= 0)).astype(np.float32)
        mk = d["cands"][lo:hi] >= 0
        mk[:, 0] = True
        rows.append(dict(x=xs, y=y, G=G, w=w, mk=mk, same=same, n=hi - lo))
    W = [(r, a, b) for r in rows for a, b in windows(r["n"])]
    for _ in range(cfg.get("epochs", 15)):
        m.train()
        np.random.shuffle(W)
        for i in range(0, len(W), 16):
            B = W[i:i + 16]
            L = max(b - a for _, a, b in B)
            x = torch.zeros(len(B), L, F)
            y = torch.full((len(B), L), -100, dtype=torch.long)
            mk = torch.zeros(len(B), L, NC, dtype=torch.bool)
            G = torch.zeros(len(B), L, NC, dtype=torch.bool)
            w = torch.zeros(len(B), L)
            same = torch.zeros(len(B), L)
            for k, (r, a, b) in enumerate(B):
                x[k, :b - a] = torch.from_numpy(r["x"][a:b])
                y[k, :b - a] = torch.from_numpy(r["y"][a:b])
                mk[k, :b - a] = torch.from_numpy(r["mk"][a:b])
                w[k, :b - a] = torch.from_numpy(r["w"][a:b])
                same[k, :b - a] = torch.from_numpy(r["same"][a:b])
                if r["G"] is not None:
                    G[k, :b - a] = torch.from_numpy(r["G"][a:b])
            lg = m(x).masked_fill(~mk, -1e9)
            if cfg.get("loss") == "set":
                lse = torch.logsumexp(lg, -1)
                good = torch.logsumexp(lg.masked_fill(~G, -1e9), -1)
                valid = G.any(-1)
                nll = torch.where(valid, lse - good, torch.zeros_like(lse))
                wt = w * valid
            else:
                nll = nn.functional.cross_entropy(lg.reshape(-1, NC), y.reshape(-1).clamp(min=0), reduction="none").reshape(len(B), L)
                valid = y >= 0
                nll = nll * valid
                wt = w * valid
            loss = (nll * wt).sum() / wt.sum().clamp(min=1)
            if cfg.get("w_pair", 0) > 0:
                # where the UST holds one note across two cells, the chance the two cells' pitches agree
                p = torch.softmax(lg, -1)
                agree = (p[:, 1:] * p[:, :-1]).sum(-1)
                pm = same[:, 1:] * (y[:, 1:] > 0) * (y[:, :-1] > 0)
                pair = -(torch.log(agree.clamp(min=1e-6)) * pm).sum() / pm.sum().clamp(min=1)
                loss = loss + cfg["w_pair"] * pair
            opt.zero_grad()
            loss.backward()
            nn.utils.clip_grad_norm_(m.parameters(), 1.0)
            opt.step()
    m.eval()
    return dict(state=m.state_dict(), mu=mu, sd=sd, F=F, NC=NC, HID=cfg.get("hid", 96), EPOCHS=cfg.get("epochs", 15), SEED=seed,
                songs=[d["id"] for d in tr], cfg=cfg, lead_residuals=F > 1046)


def logits(ck, d):
    m = M(ck["F"], ck["HID"])
    m.load_state_dict(ck["state"])
    m.eval()
    with torch.no_grad():
        x = torch.from_numpy(((d["X"][:, :ck["F"]] - ck["mu"]) / ck["sd"]).astype(np.float32))[None]
        return m(x)[0].numpy()


def log_softmax_masked(lg, cands):
    mk = cands >= 0
    mk[:, 0] = True
    z = np.where(mk, lg.astype(np.float64), -1e9)
    z = z - z.max(1, keepdims=True)
    return z - np.log(np.exp(z).sum(1, keepdims=True))
