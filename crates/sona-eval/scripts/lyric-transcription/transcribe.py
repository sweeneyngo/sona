"""Transcribe lead-vocal stems with one candidate recogniser, locally (lyrics-first 05).

    python transcribe.py CANDIDATE OUT_DIR WAV...

CANDIDATE is one of
- `whisper:<model>`: a faster-whisper model name or a local CTranslate2 folder;
- `k2:<folder>`: a sherpa-onnx ReazonSpeech transducer folder;
- `ctc:<model>`: a transformers Wav2Vec2-BERT CTC lyrics model (mrfakename/w2v-bert-2.0-music-lyrics-ctc);
- `moss:<model>`: a MOSS-Transcribe-Diarize lyric fine-tune (hcxlabs/echora-kiki); needs transformers 5
  and runs the model repository's own code.
Each WAV `.../<id>/lead.wav` gives OUT_DIR/<id>.json: the text, segments with word timestamps where the
model has them, and the wall and CPU seconds spent. Transcripts are lyric-derived: OUT_DIR must be
outside the repo.
Whisper and k2 see only the voice-activity spans of faster-whisper's Silero VAD at threshold ASR_VAD
(0.2: the default 0.5 drops 10% of dev's UST moras from singing; 0.2 keeps 98%; see vad_cover.py).
Env: ASR_THREADS (default 8), ASR_BEAM (default 5), ASR_VAD (0.2), ASR_CHUNK_S (k2 span cap, 29),
ASR_WORDS (1: word timestamps; 0 for the two-layer distilled Whisper decoders, whose alignment crashes)."""
import json
import os
import re
import sys
import time

THREADS = int(os.environ.get("ASR_THREADS", "8"))
BEAM = int(os.environ.get("ASR_BEAM", "5"))
VAD = float(os.environ.get("ASR_VAD", "0.2"))
CHUNK_S = float(os.environ.get("ASR_CHUNK_S", "29"))
WORDS = os.environ.get("ASR_WORDS", "1") == "1"


def clock():
    return time.perf_counter(), time.process_time()


def whisper_runner(model):
    from faster_whisper import WhisperModel
    m = WhisperModel(model, device="cpu", compute_type="int8", cpu_threads=THREADS)

    def run(audio):
        segs, info = m.transcribe(
            audio, language="ja", beam_size=BEAM, vad_filter=True, vad_parameters={"threshold": VAD},
            condition_on_previous_text=False, word_timestamps=WORDS)
        out = []
        for s in segs:
            out.append({"start": s.start, "end": s.end, "text": s.text,
                        "words": [{"start": w.start, "end": w.end, "word": w.word, "p": w.probability}
                                  for w in (s.words or [])]})
        return out
    return run


def k2_runner(folder):
    import sherpa_onnx
    from faster_whisper.vad import VadOptions, get_speech_timestamps

    def pick(prefix):
        names = sorted(n for n in os.listdir(folder) if n.startswith(prefix) and n.endswith(".onnx"))
        int8 = [n for n in names if "int8" in n]
        return os.path.join(folder, (int8 or names)[0])
    rec = sherpa_onnx.OfflineRecognizer.from_transducer(
        encoder=pick("encoder"), decoder=pick("decoder"), joiner=pick("joiner"),
        tokens=os.path.join(folder, "tokens.txt"), num_threads=THREADS, sample_rate=16000,
        feature_dim=80, decoding_method="modified_beam_search" if BEAM > 1 else "greedy_search",
        max_active_paths=max(BEAM, 1))

    def run(audio):
        spans = get_speech_timestamps(audio, VadOptions(threshold=VAD, max_speech_duration_s=CHUNK_S))
        out = []
        for sp in spans:
            a, b = sp["start"], sp["end"]
            st = rec.create_stream()
            st.accept_waveform(16000, audio[a:b])
            rec.decode_stream(st)
            r = st.result
            t0 = a / 16000
            ts = list(r.timestamps)
            words = [{"start": t0 + t, "end": t0 + (ts[i + 1] if i + 1 < len(ts) else (b - a) / 16000),
                      "word": tok, "p": None} for i, (tok, t) in enumerate(zip(r.tokens, ts))]
            out.append({"start": t0, "end": b / 16000, "text": r.text, "words": words})
        return out
    return run


def ctc_runner(model_id):
    """The model card's recipe: 30 s windows with 2 s of context on each side, the centres' logits
    concatenated, greedy decoding. Every emitted character keeps its frame time (50 frames/s)."""
    import numpy as np
    import torch
    from transformers import Wav2Vec2BertForCTC, Wav2Vec2BertProcessor
    torch.set_num_threads(THREADS)
    proc = Wav2Vec2BertProcessor.from_pretrained(model_id)
    model = Wav2Vec2BertForCTC.from_pretrained(model_id).eval()
    vocab = {i: t for t, i in proc.tokenizer.get_vocab().items()}
    fps, ctx, centre = 50, 2.0, 26.0

    def run(audio):
        sr = 16000
        logits = []
        for c0 in np.arange(0, len(audio) / sr, centre):
            a0 = max(0.0, c0 - ctx)
            chunk = audio[int(a0 * sr):int((c0 + centre + ctx) * sr)].astype(np.float32)
            if len(chunk) < sr // 10:
                break
            inputs = proc(chunk, sampling_rate=sr, return_tensors="pt")
            with torch.no_grad():
                lg = model(**inputs).logits[0]
            lo = int(round((c0 - a0) * fps))
            hi = lo + int(round(min(centre, len(audio) / sr - c0) * fps))
            logits.append(lg[lo:hi])
        ids = torch.cat(logits).argmax(-1).tolist()
        words, prev = [], None
        for f, i in enumerate(ids):
            if i != prev and i != 0:
                tok = vocab.get(i, "")
                if tok not in ("|", "<pad>", "<s>", "</s>", "<unk>"):
                    words.append({"start": f / fps, "end": (f + 1) / fps, "word": tok, "p": None})
            prev = i
        # one segment per pause of 1 s or more, so a reading never spans a long gap
        segs, cur = [], []
        for w in words:
            if cur and w["start"] - cur[-1]["start"] > 1.0:
                segs.append(cur)
                cur = []
            cur.append(w)
        if cur:
            segs.append(cur)
        return [{"start": c[0]["start"], "end": c[-1]["end"], "text": "".join(w["word"] for w in c), "words": c}
                for c in segs]
    return run


def moss_runner(model_id):
    """Echora's recipe: 60 s windows overlapping by 12 s, each line kept by the window that owns its
    midpoint. Backing vocals, which the model writes in parentheses, are dropped (the lead UST has none)."""
    import numpy as np
    import torch
    from transformers import AutoModelForCausalLM, AutoProcessor
    torch.set_num_threads(THREADS)
    model = AutoModelForCausalLM.from_pretrained(model_id, trust_remote_code=True, dtype=torch.float32).eval()
    proc = AutoProcessor.from_pretrained(model_id, trust_remote_code=True)
    prompt = "Transcribe the audio in Japanese. Do not translate it."
    messages = [{"role": "user", "content": [{"type": "audio", "audio": "x"}, {"type": "text", "text": prompt}]}]
    text = proc.apply_chat_template(messages, tokenize=False, add_generation_prompt=True)
    line = re.compile(r"\[(\d+(?:\.\d+)?)\](?:\[S\d+\])?(.*?)\[(\d+(?:\.\d+)?)\]")
    win, step = 60.0, 48.0

    def run(audio):
        dur = len(audio) / 16000
        starts = [0.0]
        while starts[-1] + win < dur:
            starts.append(starts[-1] + step)
        out = []
        for k, s0 in enumerate(starts):
            chunk = audio[int(s0 * 16000):int((s0 + win) * 16000)].astype(np.float32)
            inputs = proc(text=text, audio=[chunk], return_tensors="pt")
            with torch.no_grad():
                ids = model.generate(**inputs, max_new_tokens=1024, do_sample=False, repetition_penalty=1.05)
            raw = proc.tokenizer.decode(ids[0][inputs["input_ids"].shape[1]:], skip_special_tokens=True)
            lo = s0 + (win - step) / 2 if k else 0.0
            hi = s0 + win - (win - step) / 2 if k + 1 < len(starts) else dur + 1
            for m in line.finditer(raw):
                a, body, b = s0 + float(m.group(1)), m.group(2), s0 + float(m.group(3))
                if lo <= (a + b) / 2 < hi:
                    out.append({"start": a, "end": b, "text": re.sub(r"[(（][^)）]*[)）]", "", body),
                                "raw": body, "words": []})
        return out
    return run


def main():
    cand, out_dir, wavs = sys.argv[1], sys.argv[2], sys.argv[3:]
    kind, _, arg = cand.partition(":")
    os.makedirs(out_dir, exist_ok=True)
    w0, c0 = clock()
    run = {"whisper": whisper_runner, "k2": k2_runner, "ctc": ctc_runner, "moss": moss_runner}[kind](arg)
    w1, c1 = clock()
    print(f"{cand}: loaded in {w1 - w0:.1f}s wall", flush=True)
    from faster_whisper import decode_audio
    for wav in wavs:
        sid = os.path.basename(os.path.dirname(wav))
        dest = os.path.join(out_dir, sid + ".json")
        if os.path.exists(dest):
            continue
        audio = decode_audio(wav, sampling_rate=16000)
        w0, c0 = clock()
        load = os.getloadavg()[0]
        segs = run(audio)
        w1, c1 = clock()
        rec = {"id": sid, "candidate": cand, "threads": THREADS, "beam": BEAM, "vad": VAD, "chunk_s": CHUNK_S,
               "words": WORDS,
               "audio_s": len(audio) / 16000, "wall_s": w1 - w0, "cpu_s": c1 - c0,
               "load_start": load, "load_end": os.getloadavg()[0],
               "text": "".join(s["text"] for s in segs), "segments": segs}
        json.dump(rec, open(dest, "w"), ensure_ascii=False)
        print(f"{sid}: {rec['audio_s']:.0f}s audio, {rec['wall_s']:.0f}s wall, {rec['cpu_s']:.0f}s cpu, "
              f"load {load:.0f}", flush=True)


if __name__ == "__main__":
    main()
