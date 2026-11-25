"""Lyrics-first 14: CTC forced alignment of each dev song's pasted moras to a stem.

Emissions come from a CTC model over overlapping windows (30 s with 2 s of context each side, centres kept);
the whole song's token sequence is then aligned by a Viterbi in which a gap between two moras emits the blank,
and a gap between two pieces (or at either end) emits the better of the blank and the model's word filler
(`|`), so ad-libs and instrumental stretches have somewhere to go. Per mora: the first and last frame of its
tokens, and the mean log-posterior of those frames (the aligner's own score).

Writes $L14_WORK/align/<name>/<id>.json: {"frame_s", "moras": [[start_s, end_s, score], …]}. Emissions are
cached in $L14_WORK/emis/<model>-<stem>/<id>.npy. Lyric-derived: the scratchpad only.

Usage: L14_WORK=… SONA_STEMS=…/pifuyuu-stems python -I ctc_align.py --model lyrics|hira|jg|mms [--stem lead]
       [--threads 6] [ids…]
"""
import argparse, json, os, sys, time, unicodedata

import numpy as np

WORK = os.environ["L14_WORK"]
STEMS = os.environ["SONA_STEMS"]
LYRIC_ASR_HF = os.path.expanduser("~/.cache/sona/lyric-asr/hf/hub")
OWN_HF = os.path.expanduser("~/.cache/sona/line-align/hf")

MODELS = {
    # name: (HF id, kind, licence of the weights from the model card)
    "lyrics": ("mrfakename/w2v-bert-2.0-music-lyrics-ctc", "bert", "MIT"),
    "hira": ("vumichien/wav2vec2-large-xlsr-japanese-hiragana", "w2v", "Apache-2.0"),
    "jg": ("jonatasgrosman/wav2vec2-large-xlsr-53-japanese", "w2v", "Apache-2.0"),
    "mms": ("MahmoudAshraf/mms-300m-1130-forced-aligner", "w2v", "CC-BY-NC-4.0"),
}
SR = 16000
FRAME_S = 0.02
WIN_S, CTX_S = 26.0, 2.0


def local_path(repo):
    """A snapshot already downloaded by ticket 05 (read only), else our own cache."""
    d = os.path.join(LYRIC_ASR_HF, "models--" + repo.replace("/", "--"), "snapshots")
    if os.path.isdir(d) and os.listdir(d):
        return os.path.join(d, sorted(os.listdir(d))[0])
    from huggingface_hub import snapshot_download
    return snapshot_download(repo, cache_dir=OWN_HF, allow_patterns=["*.json", "*.safetensors", "*.txt", "*.model"] if repo != "jonatasgrosman/wav2vec2-large-xlsr-53-japanese" else ["*.json", "*.bin", "*.txt"])


def load_audio(path):
    import soundfile as sf, soxr
    x, sr = sf.read(path, dtype="float32", always_2d=True)
    x = x.mean(axis=1)
    return soxr.resample(x, sr, SR).astype(np.float32) if sr != SR else x


def emissions(name, audio, threads):
    import torch
    torch.set_num_threads(threads)
    repo, kind, _ = MODELS[name]
    path = local_path(repo)
    if kind == "bert":
        from transformers import Wav2Vec2BertForCTC, AutoFeatureExtractor
        model, fe = Wav2Vec2BertForCTC.from_pretrained(path).eval(), AutoFeatureExtractor.from_pretrained(path)
    else:
        from transformers import Wav2Vec2ForCTC, AutoFeatureExtractor
        model, fe = Wav2Vec2ForCTC.from_pretrained(path).eval(), AutoFeatureExtractor.from_pretrained(path)
    total = int(np.ceil(len(audio) / SR / FRAME_S))
    out = None
    win, ctx = int(WIN_S * SR), int(CTX_S * SR)
    for w0 in range(0, len(audio), win):
        a, b = max(0, w0 - ctx), min(len(audio), w0 + win + ctx)
        chunk = audio[a:b]
        if len(chunk) < SR // 2:
            chunk = np.pad(chunk, (0, SR // 2 - len(chunk)))
        inp = fe(chunk, sampling_rate=SR, return_tensors="pt")
        with torch.inference_mode():
            lp = torch.log_softmax(model(**inp).logits[0].float(), dim=-1).numpy()
        print(f"  window at {w0 / SR:6.1f}s", file=sys.stderr, flush=True)
        if out is None:
            out = np.full((total, lp.shape[1]), -1e4, dtype=np.float32)
        f_a = int(round(a / SR / FRAME_S))
        keep0 = int(round((w0 - a) / SR / FRAME_S))
        keep1 = keep0 + int(round(min(win, len(audio) - w0) / SR / FRAME_S))
        for f in range(keep0, min(keep1, lp.shape[0])):
            if 0 <= f_a + f < total:
                out[f_a + f] = lp[f]
    return out, model.config.pad_token_id


HEP = {}


def _romaji_table():
    rows = "あいうえお a i u e o|かきくけこ ka ki ku ke ko|がぎぐげご ga gi gu ge go|さしすせそ sa shi su se so|ざじずぜぞ za ji zu ze zo|" \
           "たちつてと ta chi tsu te to|だぢづでど da ji zu de do|なにぬねの na ni nu ne no|はひふへほ ha hi fu he ho|ばびぶべぼ ba bi bu be bo|" \
           "ぱぴぷぺぽ pa pi pu pe po|まみむめも ma mi mu me mo|らりるれろ ra ri ru re ro|ぁぃぅぇぉ a i u e o"
    for row in rows.split("|"):
        kana, *rom = row.split()
        HEP.update(zip(kana, rom))
    HEP.update({"や": "ya", "ゆ": "yu", "よ": "yo", "ゃ": "ya", "ゅ": "yu", "ょ": "yo", "わ": "wa", "を": "o", "ん": "n", "ゔ": "vu", "ゎ": "wa", "ー": ""})


def romaji(mora):
    if not HEP:
        _romaji_table()
    if len(mora) == 2 and mora[1] in "ゃゅょ":
        base = HEP.get(mora[0], "")
        y = HEP[mora[1]]
        if base in ("shi", "chi", "ji"):
            return base[:-1] + y[1:]
        return base[:-1] + y
    if len(mora) == 2 and mora[1] in "ぁぃぅぇぉ":
        return HEP.get(mora[0], "")[:-1] + HEP[mora[1]]
    return "".join(HEP.get(c, "") for c in mora)


def kata(c):
    return chr(ord(c) + 0x60) if "ぁ" <= c <= "ゖ" else c


def char_tokens(c, vocab):
    large = {"ぁ": "あ", "ぃ": "い", "ぅ": "う", "ぇ": "え", "ぉ": "お", "ゃ": "や", "ゅ": "ゆ", "ょ": "よ", "ゎ": "わ", "ゐ": "い", "ゑ": "え", "ゔ": "ぶ", "ぢ": "じ", "づ": "ず"}
    base = unicodedata.normalize("NFD", c)[0]
    for cand in (c, kata(c), large.get(c, c), kata(large.get(c, c)), base, kata(base)):
        if cand in vocab:
            return [vocab[cand]]
    return []


def mora_tokens(name, mora, vocab):
    if name == "mms":
        return [vocab[ch] for ch in romaji(mora) if ch in vocab]
    return [t for c in mora for t in char_tokens(c, vocab)]


def viterbi(lp, tokens, owner, gap_filler, blank, filler):
    """lp: (T, V) log-posteriors. tokens: the target token ids; owner[k]: the mora of token k; gap_filler[k]:
    whether the gap before token k (and, for k == len, after the last) may emit the filler. States 2k: the
    gap before token k; 2k+1: token k; 2K: the final gap. Returns, per token, its (first, last) frame."""
    T, K = lp.shape[0], len(tokens)
    S = 2 * K + 1
    gap_em = np.empty((T, K + 1), dtype=np.float32)
    for k in range(K + 1):
        gap_em[:, k] = np.maximum(lp[:, blank], lp[:, filler]) if (filler is not None and gap_filler[k]) else lp[:, blank]
    tok_em = lp[:, tokens]
    NEG = -1e30
    score = np.full(S, NEG)
    score[0] = gap_em[0, 0]
    score[1] = tok_em[0, 0]
    back = np.zeros((T, S), dtype=np.int8)
    tok_idx = np.arange(K)
    same_prev = np.zeros(K, dtype=bool)
    same_prev[1:] = np.asarray(tokens[1:]) == np.asarray(tokens[:-1])
    for t in range(1, T):
        stay = score
        from1 = np.concatenate(([NEG], score[:-1]))
        from2 = np.concatenate(([NEG, NEG], score[:-2]))
        # A token may come straight from the token before it unless the two are the same.
        from2[1::2][same_prev] = NEG
        from2[0::2] = NEG
        cand = np.stack([stay, from1, from2])
        arg = cand.argmax(axis=0)
        best = cand[arg, np.arange(S)]
        em = np.empty(S)
        em[0::2] = gap_em[t]
        em[1::2] = tok_em[t, tok_idx]
        score = best + em
        back[t] = arg
    s = S - 1 if score[S - 1] >= score[S - 2] else S - 2
    first = np.full(K, -1)
    last = np.full(K, -1)
    for t in range(T - 1, -1, -1):
        if s % 2 == 1:
            k = s // 2
            if last[k] < 0:
                last[k] = t
            first[k] = t
        s -= int(back[t, s])
    return first, last


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", required=True, choices=MODELS)
    ap.add_argument("--stem", default="lead")
    ap.add_argument("--threads", type=int, default=6)
    ap.add_argument("--name")
    ap.add_argument("ids", nargs="*")
    a = ap.parse_args()
    name = a.name or f"{a.model}-{a.stem}"
    ids = a.ids or sorted(f[:-5] for f in os.listdir(os.path.join(WORK, "text")))
    vocab = json.load(open(os.path.join(local_path(MODELS[a.model][0]), "vocab.json")))
    if isinstance(next(iter(vocab.values())), dict):
        vocab = next(iter(vocab.values()))
    filler = vocab.get("|")
    os.makedirs(os.path.join(WORK, "align", name), exist_ok=True)
    edir = os.path.join(WORK, "emis", f"{a.model}-{a.stem}")
    os.makedirs(edir, exist_ok=True)
    for sid in ids:
        t0 = time.time()
        text = json.load(open(os.path.join(WORK, "text", sid + ".json")))
        epath = os.path.join(edir, sid + ".npy")
        if os.path.exists(epath):
            lp = np.load(epath).astype(np.float32)
            blank = json.load(open(epath + ".json"))["blank"]
        else:
            lp, blank = emissions(a.model, load_audio(os.path.join(STEMS, sid, a.stem + ".wav")), a.threads)
            np.save(epath, lp.astype(np.float16))
            json.dump({"blank": blank}, open(epath + ".json", "w"))
        # Frames no window covered (the song's last fraction of a frame) are silence: the blank.
        lp[lp.max(axis=1) < -1e3, blank] = 0.0
        t1 = time.time()
        pieces = set(text["pieces"])
        tokens, owner, gap_filler = [], [], []
        missing = 0
        for m in text["moras"]:
            ts = mora_tokens(a.model, m["kana"], vocab)
            if not ts:
                missing += 1
                ts = [vocab.get("<unk>", vocab.get("[UNK]"))]
            for q, tk in enumerate(ts):
                tokens.append(tk)
                owner.append(m["i"])
                gap_filler.append(q == 0 and m["i"] in pieces)
        gap_filler.append(True)
        first, last = viterbi(lp, tokens, owner, gap_filler, blank, filler)
        n = len(text["moras"])
        span = [[None, None, []] for _ in range(n)]
        for k, mi in enumerate(owner):
            sp = span[mi]
            sp[0] = first[k] if sp[0] is None else min(sp[0], first[k])
            sp[1] = last[k] if sp[1] is None else max(sp[1], last[k])
            sp[2].extend(lp[first[k]:last[k] + 1, tokens[k]].tolist())
        moras = [[float(f * FRAME_S), float((l + 1) * FRAME_S), float(np.mean(sc))] for f, l, sc in span]
        json.dump({"model": a.model, "stem": a.stem, "frame_s": FRAME_S, "moras": moras, "missing_moras": missing}, open(os.path.join(WORK, "align", name, sid + ".json"), "w"))
        print(f"{sid:28} emis {t1 - t0:6.1f}s align {time.time() - t1:5.1f}s tokens {len(tokens)} frames {lp.shape[0]} missing {missing}", flush=True)


if __name__ == "__main__":
    main()
