"""Lyrics-first 17: per-mora vowel onsets from SOFA (singing-oriented forced aligner) inside each placed piece.

Each piece 14 placed (lyrics CTC starts, energy ends) is cut from the lead stem with a margin and force-aligned
by SOFA to its moras' phonemes (Japanese romaji dictionary; an optional SP after every mora). Per mora it keeps
the start of the mora's last phoneme (the vowel, or N) and of its first phoneme (the consonant).

Code: SOFA (qiuqiao/SOFA, MIT), unpacked at $SOFA_DIR. Weights: Greenleaf2001/SOFA_Models JPN_Test2_Plus
(singing; "Commercial Use: Not Approved"), at $SOFA_CKPT with its japanese-extension-sofa.txt beside it.
The checkpoint is read with torch.load(weights_only=True).

Writes $L17_WORK/onsets/sofa-vowel/<id>.json and sofa-first/<id>.json ({"on": […]}) and
$L17_WORK/sofa/<id>.json (per-piece confidence). Lyric-derived: the scratchpad only.

Usage: L14_WORK=… L17_WORK=… SONA_STEMS=… SOFA_DIR=… SOFA_CKPT=… venv/bin/python -I sofa_align.py [--threads 4]
       [--margin-before 0.4] [--margin-after 0.3] [ids…]
"""
import argparse, json, os, sys, time

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
W14, W17 = os.environ["L14_WORK"], os.environ["L17_WORK"]
STEMS = os.environ["SONA_STEMS"]
SOFA_DIR, CKPT = os.environ["SOFA_DIR"], os.environ["SOFA_CKPT"]
TIMES = os.environ.get("L17_TIMES", "lyrics-lead-energy")

HEP_ROWS = ("あいうえお a i u e o|かきくけこ ka ki ku ke ko|がぎぐげご ga gi gu ge go|さしすせそ sa shi su se so|"
            "ざじずぜぞ za ji zu ze zo|たちつてと ta chi tsu te to|だぢづでど da ji zu de do|なにぬねの na ni nu ne no|"
            "はひふへほ ha hi fu he ho|ばびぶべぼ ba bi bu be bo|ぱぴぷぺぽ pa pi pu pe po|まみむめも ma mi mu me mo|"
            "らりるれろ ra ri ru re ro|ぁぃぅぇぉ a i u e o")
HEP = {}
for row in HEP_ROWS.split("|"):
    kana, *rom = row.split()
    HEP.update(zip(kana, rom))
HEP.update({"や": "ya", "ゆ": "yu", "よ": "yo", "わ": "wa", "を": "o", "ん": "N", "ゔ": "vu"})
SMALL_Y = {"ゃ": "a", "ゅ": "u", "ょ": "o"}
SMALL_V = {"ぁ": "a", "ぃ": "i", "ぅ": "u", "ぇ": "e", "ぉ": "o"}


def romaji(mora):
    """The SOFA dictionary's syllable for a mora (Hepburn, with ん as N and the compound kana)."""
    if len(mora) == 2 and mora[1] in SMALL_Y:
        base = HEP[mora[0]]
        if base in ("shi", "chi", "ji"):
            return base[:-1] + SMALL_Y[mora[1]]
        return base[:-1] + "y" + SMALL_Y[mora[1]]
    if len(mora) == 2 and mora[1] in SMALL_V:
        v = SMALL_V[mora[1]]
        if mora[0] == "う":
            return "w" + v
        base = HEP[mora[0]]
        return (base[0] if base[0] in "tdfv" else base[:-1]) + v
    return HEP[mora]


def phones(mora, dictionary):
    r = romaji(mora)
    if r in dictionary:
        return dictionary[r]
    # Fallback: the vowel alone (counted in the report).
    return [r[-1]] if r[-1] in "aiueo" else ["N"]


def load_model(threads):
    import torch
    torch.set_num_threads(threads)
    sys.path.insert(0, SOFA_DIR)
    from modules.task.forced_alignment import LitForcedAlignmentTask
    ck = torch.load(CKPT, map_location="cpu", weights_only=True)
    model = LitForcedAlignmentTask(**ck["hyper_parameters"])
    model.load_state_dict(ck["state_dict"])
    model.eval()
    model.set_inference_mode("force")
    model.on_predict_start()
    return model


def align_segment(model, wav, sr, ph_seq):
    import torch
    from einops import repeat
    w = torch.from_numpy(wav)
    mel = model.get_melspec(w).detach().unsqueeze(0)
    mel = (mel - mel.mean()) / mel.std()
    mel = repeat(mel, "B C T -> B C (T N)", N=model.melspec_config["scale_factor"])
    ph_seq_pred, intervals, _w, _wi, conf, _c, _f = model._infer_once(mel, len(wav) / sr, ph_seq, None, None, False, False)
    return list(ph_seq_pred), np.asarray(intervals), float(conf)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--threads", type=int, default=4)
    ap.add_argument("--margin-before", type=float, default=0.4)
    ap.add_argument("--margin-after", type=float, default=0.3)
    ap.add_argument("--suffix", default="")
    ap.add_argument("ids", nargs="*")
    a = ap.parse_args()
    import soundfile as sf, soxr
    dpath = os.path.join(os.path.dirname(CKPT), "japanese-extension-sofa.txt")
    dictionary = {l.split("\t")[0].strip(): l.split("\t")[1].strip().split(" ") for l in open(dpath, encoding="utf-8").read().strip().split("\n")}
    model = load_model(a.threads)
    sr = model.melspec_config["sample_rate"]
    names = ("sofa-vowel" + a.suffix, "sofa-first" + a.suffix)
    for n in names:
        os.makedirs(os.path.join(W17, "onsets", n), exist_ok=True)
    os.makedirs(os.path.join(W17, "sofa"), exist_ok=True)
    ids = a.ids or sorted(f[:-5] for f in os.listdir(os.path.join(W14, "text")))
    for sid in ids:
        t_start = time.time()
        text = json.load(open(os.path.join(W14, "text", sid + ".json")))
        times = json.load(open(os.path.join(W14, "times", TIMES, sid + ".json")))
        x, xsr = sf.read(os.path.join(STEMS, sid, "lead.wav"), dtype="float32", always_2d=True)
        x = x.mean(axis=1)
        if xsr != sr:
            x = soxr.resample(x, xsr, sr).astype(np.float32)
        moras = text["moras"]
        n = len(moras)
        vowel, first = [None] * n, [None] * n
        pieces = times["pieces"]
        bounds = [p[0] for p in pieces] + [n]
        log, fallback = [], 0
        for p, nxt in zip(pieces, bounds[1:]):
            m0, t0, t1 = p[0], p[1], p[2]
            s0 = max(0.0, t0 - a.margin_before)
            s1 = min(len(x) / sr, t1 + a.margin_after)
            if s1 - s0 < 0.2 or nxt <= m0:
                continue
            seg = x[int(s0 * sr):int(s1 * sr)]
            ph_seq, owner, is_last = ["SP"], [None], [False]
            for i in range(m0, nxt):
                ph = phones(moras[i]["kana"], dictionary)
                if romaji(moras[i]["kana"]) not in dictionary:
                    fallback += 1
                for q, h in enumerate(ph):
                    ph_seq.append(h)
                    owner.append(i)
                    is_last.append(q == len(ph) - 1)
                ph_seq.append("SP")
                owner.append(None)
                is_last.append(False)
            want = [(o, l) for h, o, l in zip(ph_seq, owner, is_last) if h != "SP"]
            pred, iv, conf = align_segment(model, seg, sr, ph_seq)
            if len(pred) != len(want):
                log.append([m0, None])
                continue
            for (o, last), (b, _e) in zip(want, iv):
                if first[o] is None:
                    first[o] = float(s0 + b)
                if last:
                    vowel[o] = float(s0 + b)
            log.append([m0, conf])
        json.dump({"on": vowel}, open(os.path.join(W17, "onsets", names[0], sid + ".json"), "w"))
        json.dump({"on": first}, open(os.path.join(W17, "onsets", names[1], sid + ".json"), "w"))
        json.dump({"pieces": log, "fallback_moras": fallback}, open(os.path.join(W17, "sofa", sid + a.suffix + ".json"), "w"))
        print(f"{sid:28} pieces {len(pieces)} fallback {fallback} failed {sum(1 for _, c in log if c is None)} {time.time() - t_start:6.1f}s", flush=True)


if __name__ == "__main__":
    main()
