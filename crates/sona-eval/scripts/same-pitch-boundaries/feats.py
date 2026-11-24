"""Per-song frame features of a stem for the same-pitch boundary detector.

usage: python feats.py OUT_DIR STEM(lead|vocals) SONG_ID...
Writes OUT_DIR/<id>.<stem>.npz: t (frame centre s), logmel (40xT), mfcc (13xT), rms (T).
"""
import sys, os
import numpy as np, soundfile as sf, librosa

STEMS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "data", "pifuyuu-stems")
SR, NFFT, HOP = 16000, 512, 80

out, stem = sys.argv[1], sys.argv[2]
os.makedirs(out, exist_ok=True)
for sid in sys.argv[3:]:
    dst = os.path.join(out, f"{sid}.{stem}.npz")
    if os.path.exists(dst):
        continue
    y, sr = sf.read(os.path.join(STEMS, sid, f"{stem}.wav"), dtype="float32", always_2d=True)
    y = librosa.resample(y.mean(axis=1), orig_sr=sr, target_sr=SR)
    S = np.abs(librosa.stft(y, n_fft=NFFT, hop_length=HOP, center=True)) ** 2
    mel = librosa.feature.melspectrogram(S=S, sr=SR, n_mels=40, fmin=60, fmax=7600)
    logmel = np.log(mel + 1e-8)
    mfcc = librosa.feature.mfcc(S=logmel, n_mfcc=13)
    rms = np.sqrt(S.sum(axis=0) / NFFT)
    t = np.arange(S.shape[1]) * HOP / SR
    np.savez_compressed(dst + ".tmp.npz", t=t.astype(np.float32), logmel=logmel.astype(np.float32),
                        mfcc=mfcc.astype(np.float32), rms=rms.astype(np.float32))
    os.replace(dst + ".tmp.npz", dst)
    print(sid, S.shape, flush=True)
