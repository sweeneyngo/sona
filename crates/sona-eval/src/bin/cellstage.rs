//! The learned cell stage, ported (ADR-0026), scored per grid cell against the lead UST over the UST's
//! span — the prototype's own metric — with the stage on sona's estimated grid phase (what ships) and,
//! for comparison with the prototype's 78.8%, on the UST's true phase.
//!
//! ```sh
//! ORT_DYLIB_PATH=… SONA_GAME_DIR=… cargo run --release -p sona-eval --features game --bin sona-eval-cellstage -- \
//!     --model ~/.cache/sona/cellstage-D [--edit-cost] [--split holdout3 --reason "…" --reread]
//! ```
//!
//! `--edit-cost` adds the OpenUtau edit cost (`sona_eval::correction::edit_cost`) against the lead UST
//! on its own grid, per song and pooled, for sona's default pYIN grid mode on the vocals, GAME's lead
//! notes on the onset phase, and the learned stage on the onset phase (what ships).
//!
//! The GAME runs and sona's pYIN grid notes are cached under `target/ust-dev-cache/` (or
//! `$SONA_DEV_CACHE`), keyed like `sona-eval-ust-dev`'s: song, a fingerprint of each stem, the GAME
//! release folder, the GAME settings (seed included) and the transcriber's settings. Delete the cache
//! after changing GAME's or the octave check's code.
//!
//! `--fit-phrase-lyrics` (opt-in, utau-edit-cost 07) adds the edit cost of GAME's lead notes and of the
//! stage after `sona_core::lyric_fit::fit_phrase`, as a user who pastes each phrase's lyric onto its notes
//! would get it: a phrase is a run of touching notes, and its moras are the lead UST's notes sung inside
//! its span (by midpoint). The default run is unchanged.
//!
//! `--dump DIR` also writes each song's inputs and outputs to `DIR/<id>.json` for offline analysis:
//! the lead UST's notes with their lyrics, the three outputs and their edit costs, the stage's cells
//! (candidates, sources, probabilities), each shipped note's confidence, GAME's raw notes and the
//! vocals' pitch track. The dump holds UST content: keep it out of the repository.
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use sona_core::cellstage::{cell_features, cells_to_notes, choose_class, snap_notes, EncoderChunk, StageInputs, StageNote};
use sona_core::lyric_fit::{fit_phrase, mora_of, CellNote, Mora};
use sona_core::tempo::grid_phase;
use sona_core::game::{cell_stage_notes, cells_covering, grid_segments, phase_of, CellModel, GameModel, GameParams, StageRuns};
use sona_core::{Annotation, GridMode, PYin, Producer, Transcriber};
use sona_eval::correction::edit_cost::{self, edit_cost, CellGrid, EditCost};
use sona_eval::correction::EditNote;
use sona_eval::holdout::authorize;
use sona_eval::quantize::grid_spacing_seconds;
use sona_eval::ust_corpus::{manifest_path, stems_dir, Manifest, SongUsts};
use sona_eval::ust::BlockKind;
use sona_eval::{ust, wav};
use serde_json::json;

type Notes = Vec<(f64, f64, i32)>;

/// The notes a user gets by pasting each phrase's lyric onto it: phrases are runs of touching notes on
/// the grid at `anchor`; each takes the moras of `sung` (onset, offset, mora) whose midpoint lies in it.
fn fit_to_sung_phrases(notes: &Notes, sung: &[(f64, f64, Mora)], spacing: f64, anchor: f64) -> Notes {
    let cell = |t: f64| ((t - anchor) / spacing).round() as i64;
    let mut twice_mid: Vec<(i64, Mora)> = sung.iter().map(|&(a, b, m)| (cell(a) + cell(b), m)).collect();
    twice_mid.sort();
    let cells: Vec<CellNote> = notes.iter().map(|&(a, b, p)| CellNote { start: cell(a), end: cell(b), pitch: p }).collect();
    let mut fitted = Vec::with_capacity(cells.len());
    let mut i = 0;
    while i < cells.len() {
        let mut j = i + 1;
        while j < cells.len() && cells[j].start <= cells[j - 1].end {
            j += 1;
        }
        let (from, to) = (cells[i].start, cells[j - 1].end);
        let lo = twice_mid.partition_point(|&(m, _)| m < 2 * from);
        let hi = twice_mid.partition_point(|&(m, _)| m < 2 * to);
        let moras: Vec<Mora> = twice_mid[lo..hi].iter().map(|&(_, m)| m).collect();
        fitted.extend(fit_phrase(&cells[i..j], &moras));
        i = j;
    }
    fitted.iter().map(|n| (anchor + n.start as f64 * spacing, anchor + n.end as f64 * spacing, n.pitch)).collect()
}

fn cells(reference: &Notes, est: &Notes, spacing: f64, anchor: f64) -> (usize, usize) {
    let at = |ns: &Notes, t: f64| -> i32 {
        let i = ns.partition_point(|n| n.0 <= t);
        if i > 0 && ns[i - 1].0 <= t && t < ns[i - 1].1 {
            ns[i - 1].2
        } else {
            -1
        }
    };
    let (first, last) = (reference[0].0, reference[reference.len() - 1].1);
    let mut k = ((first - anchor) / spacing).round_ties_even() as i64;
    let (mut ok, mut n) = (0, 0);
    while anchor + (k as f64 + 0.5) * spacing < last {
        let t = anchor + (k as f64 + 0.5) * spacing;
        ok += usize::from(at(reference, t) == at(est, t));
        n += 1;
        k += 1;
    }
    (ok, n)
}

fn cache_dir() -> PathBuf {
    std::env::var_os("SONA_DEV_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/ust-dev-cache"))
}

fn settings_key(p: &impl std::fmt::Debug) -> String {
    let s = format!("{p:?}");
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn fingerprint(samples: &[f32]) -> u64 {
    samples.iter().step_by(997).fold(samples.len() as u64, |h, x| h.rotate_left(5) ^ u64::from(x.to_bits()))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::create_dir_all(cache_dir()).map_err(|e| e.to_string())?;
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

fn put_notes(b: &mut Vec<u8>, notes: &[StageNote]) {
    b.extend((notes.len() as u64).to_le_bytes());
    for n in notes {
        b.extend(n.onset_s.to_le_bytes());
        b.extend(n.offset_s.to_le_bytes());
        b.extend(n.midi.to_le_bytes());
        b.push(u8::from(n.voiced));
    }
}

fn put_f32s(b: &mut Vec<u8>, v: &[f32]) {
    b.extend((v.len() as u64).to_le_bytes());
    for x in v {
        b.extend(x.to_le_bytes());
    }
}

fn put_chunks(b: &mut Vec<u8>, chunks: &[EncoderChunk]) {
    b.extend((chunks.len() as u64).to_le_bytes());
    for c in chunks {
        b.extend(c.t0.to_le_bytes());
        b.extend((c.frames as u64).to_le_bytes());
        b.extend((c.dim as u64).to_le_bytes());
        put_f32s(b, &c.data);
    }
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (head, rest) = self.0.split_first_chunk::<N>()?;
        self.0 = rest;
        Some(*head)
    }
    fn u64(&mut self) -> Option<u64> {
        self.take().map(u64::from_le_bytes)
    }
    fn f64(&mut self) -> Option<f64> {
        self.take().map(f64::from_le_bytes)
    }
    fn f32(&mut self) -> Option<f32> {
        self.take().map(f32::from_le_bytes)
    }
    fn notes(&mut self) -> Option<Vec<StageNote>> {
        (0..self.u64()?).map(|_| Some(StageNote { onset_s: self.f64()?, offset_s: self.f64()?, midi: self.f32()?, voiced: self.take::<1>()?[0] != 0 })).collect()
    }
    fn f32s(&mut self) -> Option<Vec<f32>> {
        (0..self.u64()?).map(|_| self.f32()).collect()
    }
    fn chunks(&mut self) -> Option<Vec<EncoderChunk>> {
        (0..self.u64()?).map(|_| Some(EncoderChunk { t0: self.f64()?, frames: self.u64()? as usize, dim: self.u64()? as usize, data: self.f32s()? })).collect()
    }
}

const RUNS_MAGIC: &[u8; 8] = b"sonaSR01";

fn runs_to_bytes(r: &StageRuns) -> Vec<u8> {
    let mut b = RUNS_MAGIC.to_vec();
    put_notes(&mut b, &r.lead_notes);
    put_f32s(&mut b, &r.lead_ratios);
    put_chunks(&mut b, &r.lead_frames);
    put_notes(&mut b, &r.vocals_notes);
    put_chunks(&mut b, &r.vocals_frames);
    b
}

fn runs_from_bytes(bytes: &[u8]) -> Option<StageRuns> {
    let mut r = Reader(bytes.strip_prefix(RUNS_MAGIC)?);
    let runs = StageRuns { lead_notes: r.notes()?, lead_ratios: r.f32s()?, lead_frames: r.chunks()?, vocals_notes: r.notes()?, vocals_frames: r.chunks()? };
    r.0.is_empty().then_some(runs)
}

fn stage_runs_cached(game: &GameModel, game_dir: &Path, id: &str, lead: &[f32], vocals: &[f32], sr: u32) -> Result<StageRuns, String> {
    let tag = game_dir.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let path = cache_dir().join(format!("{id}.stageruns-{tag}-{}-{:016x}-{:016x}.bin", settings_key(&game.params), fingerprint(lead), fingerprint(vocals)));
    if let Some(runs) = std::fs::read(&path).ok().and_then(|b| runs_from_bytes(&b)) {
        return Ok(runs);
    }
    eprintln!("  GAME for {id} (cached after this run)…");
    let runs = StageRuns::compute(game, lead, vocals, sr).map_err(|e| e.to_string())?;
    write_atomic(&path, &runs_to_bytes(&runs))?;
    Ok(runs)
}

fn transcribe_cached(t: &Transcriber, id: &str, samples: &[f32], sr: u32, bpm: f64) -> Result<Annotation, String> {
    let path = cache_dir().join(format!("{id}.cellstage-pyin-{}-{bpm}-{:016x}.json", settings_key(t), fingerprint(samples)));
    if let Some(a) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()) {
        return Ok(a);
    }
    let a = t.transcribe(&PYin::default(), samples, sr, bpm, Producer::Pyin);
    write_atomic(&path, serde_json::to_string(&a).map_err(|e| e.to_string())?.as_bytes())?;
    Ok(a)
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let model_dir = PathBuf::from(flag("--model").ok_or("--model DIR (an exported cell stage) is required")?);
    let split = flag("--split").unwrap_or_else(|| "dev".into());
    let manifest = Manifest::load(&manifest_path()).map_err(|e| e.to_string())?;
    if split != "dev" {
        authorize(&manifest, &split, "sona-eval-cellstage", flag("--reason").as_deref(), args.iter().any(|a| a == "--reread"))?;
    }
    let game_dir = std::env::var("SONA_GAME_DIR").map_err(|_| "set SONA_GAME_DIR")?;
    let game = GameModel::load(Path::new(&game_dir), GameParams::default()).map_err(|e| e.to_string())?;
    let model = CellModel::load(&model_dir, GameParams::default().threads).map_err(|e| e.to_string())?;
    let (cache, stems) = (ust::corpus_dir(), stems_dir());
    let songs: Vec<_> = manifest
        .songs
        .iter()
        .filter(|s| s.split.as_deref() == Some(split.as_str()) && s.stratum == "vocaloid-original")
        .filter(|s| s.alignment_status.as_deref().is_some_and(|a| a.starts_with("aligned")) && s.alignment_offset_s.is_some())
        .filter(|s| stems.join(&s.id).join("lead.wav").is_file() && stems.join(&s.id).join("vocals.wav").is_file())
        .filter(|s| std::env::var("SONA_ONLY").map_or(true, |only| only.split(',').any(|o| o == s.id)))
        .collect();
    let mut tot = [(0usize, 0usize); 6];
    let with_edit_cost = args.iter().any(|a| a == "--edit-cost");
    let with_fit = args.iter().any(|a| a == "--fit-phrase-lyrics");
    let mut costs: [Vec<(String, EditCost)>; 5] = Default::default();
    let dump = flag("--dump").map(PathBuf::from);
    if let Some(dir) = &dump {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    println!("{:28} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}", "song", "lead/est", "stage/est", "lead/ons", "stage/ons", "lead/UST", "stage/UST");
    for song in songs {
        let offset = song.alignment_offset_s.expect("filtered");
        let usts = SongUsts::load(song, &cache).map_err(|e| e.to_string())?;
        let bpm = song.bpm_from_ust.unwrap_or(usts.lead[0].tempo);
        let division = song.grid_division.unwrap_or(16);
        let spacing = grid_spacing_seconds(bpm, f64::from(division));
        let reference: Notes = usts.reference(offset).notes.iter().map(|r| (r.onset_s, r.offset_s, i32::from(r.midi_pitch))).collect();
        let (lead, sr) = wav::read_mono(&stems.join(&song.id).join("lead.wav")).map_err(|e| e.to_string())?;
        let (vocals, _) = wav::read_mono(&stems.join(&song.id).join("vocals.wav")).map_err(|e| e.to_string())?;
        let t = Transcriber { grid: Some(GridMode { division, ..GridMode::sixteenths() }), ..Transcriber::default() };
        let a = transcribe_cached(&t, &song.id, &vocals, sr, bpm)?;
        let sona: Notes = a.notes.iter().map(|n| (n.onset_s, n.offset_s, i32::from(n.midi_pitch))).collect();
        let estimated = phase_of(&sona, spacing);
        let f0: Vec<(f64, f64)> = a.f0_contour.iter().map(|f| (f.time_s, f64::from(f.frequency_hz))).collect();
        let rms: Vec<(f64, f64)> = a.energy_envelope.iter().map(|e| (e.time_s, f64::from(e.rms))).collect();
        let duration = vocals.len() as f64 / f64::from(sr);
        let runs = stage_runs_cached(&game, Path::new(&game_dir), &song.id, &lead, &vocals, sr)?;
        let truth = offset.rem_euclid(spacing);
        let off = |p: f64| {
            let d = (p - truth).rem_euclid(spacing) / spacing;
            if d > 0.5 { d - 1.0 } else { d }
        };
        let onset = grid_phase(&vocals, sr, spacing).unwrap_or(estimated);
        let t_on = Transcriber { grid: Some(GridMode { division, anchor_s: Some(onset), ..GridMode::sixteenths() }), ..Transcriber::default() };
        let a_on = transcribe_cached(&t_on, &song.id, &vocals, sr, bpm)?;
        let sona_on: Notes = a_on.notes.iter().map(|n| (n.onset_s, n.offset_s, i32::from(n.midi_pitch))).collect();
        let mut row = [(0usize, 0usize); 6];
        let grid = CellGrid { spacing_s: spacing, anchor_s: offset, cells_per_measure: division };
        let edit_notes = |ns: &Notes| ns.iter().map(|&(a, b, p)| EditNote { onset_s: a, offset_s: b, midi_pitch: f64::from(p) }).collect::<Vec<_>>();
        if with_edit_cost {
            costs[0].push((song.id.clone(), edit_cost(&edit_notes(&reference), &edit_notes(&sona), &grid)));
        }
        for (j, (anchor, notes)) in [(estimated, &sona), (onset, &sona_on), (offset.rem_euclid(spacing), &sona)].into_iter().enumerate() {
            let lead_only = snap_notes(&runs.lead_notes, spacing, anchor);
            let stage = cell_stage_notes(&model, &runs, notes, &f0, &rms, spacing, anchor, cells_covering(duration, spacing, anchor)).map_err(|e| e.to_string())?;
            row[2 * j] = cells(&reference, &lead_only, spacing, offset);
            row[2 * j + 1] = cells(&reference, &stage, spacing, offset);
            if with_edit_cost && j == 1 {
                costs[1].push((song.id.clone(), edit_cost(&edit_notes(&reference), &edit_notes(&lead_only), &grid)));
                costs[2].push((song.id.clone(), edit_cost(&edit_notes(&reference), &edit_notes(&stage), &grid)));
                if with_fit {
                    let sung: Vec<(f64, f64, Mora)> = usts
                        .lead
                        .iter()
                        .flat_map(|u| u.timeline().into_iter().filter(|p| p.kind == BlockKind::Note).map(move |p| (p.start_s + offset, p.end_s + offset, mora_of(&u.notes[p.i].lyric))))
                        .collect();
                    for (k, ns) in [(3, &lead_only), (4, &stage)] {
                        let fitted = fit_to_sung_phrases(ns, &sung, spacing, anchor);
                        costs[k].push((song.id.clone(), edit_cost(&edit_notes(&reference), &edit_notes(&fitted), &grid)));
                    }
                }
            }
            if let (Some(dir), 1) = (&dump, j) {
                let outputs = [("pyin", &sona), ("lead", &lead_only), ("stage", &stage)];
                let costs: Vec<_> = outputs.iter().map(|(name, ns)| (*name, edit_cost(&edit_notes(&reference), &edit_notes(ns), &grid))).collect();
                let inputs = StageInputs { spacing, anchor, lead_notes: &runs.lead_notes, lead_ratios: &runs.lead_ratios, vocals_notes: &runs.vocals_notes, sona_notes: notes, lead_frames: &runs.lead_frames, vocals_frames: &runs.vocals_frames, f0: &f0, rms: &rms };
                let song_dump = dump_song(&model, &inputs, cells_covering(duration, spacing, anchor), &stage, &lead, sr, &usts, offset)?;
                let value = json!({
                    "id": song.id, "bpm": bpm, "division": division, "spacing": spacing, "ust_anchor": offset, "onset_phase": anchor,
                    "reference": reference, "outputs": outputs.iter().map(|(name, ns)| (name.to_string(), json!(ns))).collect::<serde_json::Map<_, _>>(),
                    "edit_cost": costs.iter().map(|(name, c)| (name.to_string(), json!(c.counts))).collect::<serde_json::Map<_, _>>(),
                    "sona_onset": notes, "f0": f0, "rms": rms, "stage_detail": song_dump,
                });
                let path = dir.join(format!("{}.json", song.id));
                std::fs::write(&path, value.to_string()).map_err(|e| format!("{}: {e}", path.display()))?;
            }
        }
        let pct = |(o, n): (usize, usize)| 100.0 * o as f64 / n.max(1) as f64;
        println!("{:28} {:>8.1}% {:>8.1}% {:>8.1}% {:>8.1}% {:>8.1}% {:>8.1}%   phase est {:+.3} ons {:+.3} cells", song.id, pct(row[0]), pct(row[1]), pct(row[2]), pct(row[3]), pct(row[4]), pct(row[5]), off(estimated), off(onset));
        for (t, r) in tot.iter_mut().zip(row) {
            t.0 += r.0;
            t.1 += r.1;
        }
    }
    let pct = |(o, n): (usize, usize)| 100.0 * o as f64 / n.max(1) as f64;
    println!("{:28} {:>8.2}% {:>8.2}% {:>8.2}% {:>8.2}% {:>8.2}% {:>8.2}%   ({} cells)", "ALL", pct(tot[0]), pct(tot[1]), pct(tot[2]), pct(tot[3]), pct(tot[4]), pct(tot[5]), tot[0].1);
    if with_edit_cost {
        let titles = [
            "pYIN grid mode on the vocals (its own phase)",
            "GAME lead + octave rule, onset phase",
            "learned cell stage, onset phase (ships)",
            "GAME lead + octave rule, each phrase's lyric fitted (opt-in)",
            "learned cell stage, each phrase's lyric fitted (opt-in)",
        ];
        for (title, rows) in titles.iter().zip(&costs).filter(|(_, rows)| !rows.is_empty()) {
            print!("{}", edit_cost::report(title, rows));
        }
    }
    Ok(())
}

/// The stage's cells and notes in detail, for `--dump`: candidates, sources, class probabilities and
/// each shipped note's confidence (scored as `sona transcribe` scores them, on the lead's energy).
#[allow(clippy::too_many_arguments)]
fn dump_song(model: &CellModel, inputs: &StageInputs<'_>, (k0, n): (i64, usize), stage: &Notes, lead: &[f32], sr: u32, usts: &SongUsts, offset: f64) -> Result<serde_json::Value, String> {
    let (x, cands, sources) = cell_features(inputs, k0, n);
    let logits = model.logits(&x, n).map_err(|e| e.to_string())?;
    let classes = choose_class(&logits, &cands);
    if &cells_to_notes(&classes, &cands, &sources, k0, inputs.spacing, inputs.anchor) != stage {
        return Err("the dump's stage notes differ from cell_stage_notes".into());
    }
    let nc = cands.first().map_or(6, |c| c.len());
    let probs: Vec<Vec<f32>> = logits
        .chunks(nc)
        .map(|row| {
            let top = row.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let e: Vec<f32> = row.iter().map(|v| (v - top).exp()).collect();
            let z: f32 = e.iter().sum();
            e.iter().map(|v| v / z).collect()
        })
        .collect();
    let shipped: Vec<(f64, f64, u8)> = stage.iter().filter_map(|&(a, b, p)| u8::try_from(p).ok().filter(|p| *p <= 127).map(|p| (a, b, p))).collect();
    let envelope = Transcriber::default().onset.detect(lead, sr).energy_envelope;
    let scored = Transcriber::default().scorer.score_all(&grid_segments(&shipped), &envelope);
    let ust_notes: Vec<_> = usts
        .lead
        .iter()
        .enumerate()
        .flat_map(|(part, u)| {
            u.timeline().into_iter().filter(|p| p.kind != BlockKind::Rest).map(move |p| {
                json!({"part": part, "on": p.start_s + offset, "off": p.end_s + offset, "midi": p.note_num, "lyric": u.notes[p.i].lyric, "kind": format!("{:?}", p.kind), "ticks": [p.start_tick, p.end_tick]})
            })
        })
        .collect();
    let stage_notes = |ns: &[StageNote]| ns.iter().map(|n| (n.onset_s, n.offset_s, n.midi, n.voiced)).collect::<Vec<_>>();
    Ok(json!({
        "k0": k0, "classes": classes, "cands": cands, "sources": sources, "probs": probs,
        "confidence": scored.iter().map(|n| (n.onset_s, n.offset_s, n.midi_pitch, n.confidence, [n.sub_scores.pitch_stability, n.sub_scores.voicing, n.sub_scores.onset_clarity, n.sub_scores.offset_clarity])).collect::<Vec<_>>(),
        "ust": ust_notes, "game_lead": stage_notes(inputs.lead_notes), "lead_ratios": inputs.lead_ratios, "game_vocals": stage_notes(inputs.vocals_notes),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runs() -> StageRuns {
        let note = |a: f64, b: f64, m: f32, v: bool| StageNote { onset_s: a, offset_s: b, midi: m, voiced: v };
        let chunk = |t0: f64, frames: usize, dim: usize| EncoderChunk { t0, frames, dim, data: (0..frames * dim).map(|i| i as f32 * 0.37 - 1.5).collect() };
        StageRuns {
            lead_notes: vec![note(0.1, 0.4, 60.25, true), note(0.4, 0.9, 0.0, false)],
            lead_ratios: vec![0.5, f32::MIN_POSITIVE],
            lead_frames: vec![chunk(0.0, 3, 2), chunk(19.7, 2, 2)],
            vocals_notes: vec![note(0.12, 0.38, 59.9, true)],
            vocals_frames: vec![chunk(0.0, 4, 2)],
        }
    }

    fn same(a: &StageRuns, b: &StageRuns) -> bool {
        a.lead_notes == b.lead_notes && a.lead_ratios == b.lead_ratios && a.lead_frames == b.lead_frames && a.vocals_notes == b.vocals_notes && a.vocals_frames == b.vocals_frames
    }

    #[test]
    fn cached_game_runs_read_back_exactly() {
        let r = runs();
        let back = runs_from_bytes(&runs_to_bytes(&r)).expect("reads back");
        assert!(same(&r, &back));
    }

    #[test]
    fn a_torn_or_foreign_cache_file_is_recomputed_not_trusted() {
        let bytes = runs_to_bytes(&runs());
        assert!(runs_from_bytes(&bytes[..bytes.len() - 1]).is_none(), "truncated");
        assert!(runs_from_bytes(&[bytes.as_slice(), &[0]].concat()).is_none(), "trailing bytes");
        assert!(runs_from_bytes(&bytes[8..]).is_none(), "no magic");
    }
}
