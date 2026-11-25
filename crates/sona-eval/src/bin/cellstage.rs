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
//! `--merge-unheard-pieces` (opt-in, utau-edit-cost 11) adds the edit cost of GAME's lead notes and of
//! the stage after `sona_core::unheard_pieces::merge_unheard_pieces`, with GAME's run on the full
//! vocals as the second listen; with `--fit-phrase-lyrics` too, also merged and then fitted.
//! `--trim-releases` (opt-in, utau-edit-cost 12) adds the edit cost of GAME's lead notes and of the stage
//! after `sona_core::release::trim_releases` on the vocals' energy envelope (the stage's row shows why it
//! is not applied there). The default run is unchanged.
//! `--settle-near-ties` (opt-in, utau-edit-cost 18) adds the edit cost of GAME's lead notes and of the
//! stage after `sona_core::near_ties::settle_near_ties`, with GAME's run on the full vocals as the second
//! listen; with `--trim-releases` too, GAME's lead notes settled and then trimmed, and with
//! `--merge-unheard-pieces` too, the stage settled and then merged.
//!
//! `--lyric-placement` (with `--edit-cost`; lyrics-first 01) adds the lyric placement
//! (`sona_eval::correction::lyric_placement`) and the edit cost of GAME's lead notes and of the stage when
//! the lead UST's own moras land on them: pasted in order with no fit (what the app does today), and fitted
//! by `sona_core::lyric_fit` with the phrases taken from runs of the output's touching notes (07 as ported),
//! from the UST's phrases, or from the whole song; "rests hold moras" uses `fit_span_moras`. With
//! `--lyric-lines DIR` too, the phrases also come from a lyric text's lines (and lines and spaces):
//! `DIR/<id>.json` holds `line_starts` and `space_starts`, the indices of the UST notes that begin one
//! (`scripts/lyric-placement/align_lines.py`; lyric-derived, keep it out of the repo). With
//! `--lyric-line-times DIR` (lyrics-first 14), the lines and pieces are placed in time by an aligner instead
//! of the UST: `DIR/<id>.json` holds `lines` and `pieces`, each `[first UST note, start s, end s, …]`, and
//! each is fitted by `sona_core::lyric_fit::fit_lines` (`scripts/line-alignment/`; lyric-derived too).
//!
//! `--dump DIR` also writes each song's inputs and outputs to `DIR/<id>.json` for offline analysis:
//! the lead UST's notes with their lyrics, the three outputs and their edit costs, the stage's cells
//! (candidates, sources, probabilities), each shipped note's confidence, GAME's raw notes and the
//! vocals' pitch track. The dump holds UST content: keep it out of the repository.
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use sona_core::cellstage::{cell_features, cells_to_notes, choose_class, snap_notes, EncoderChunk, StageInputs, StageNote};
use sona_core::lyric_fit::{fit_lines, fit_phrase, fit_phrase_moras, fit_span_moras, mora_of, CellNote, LineSpan, Mora};
use sona_core::near_ties::{settle_near_ties, NearTieRule};
use sona_core::release::{trim_releases, ReleaseRule};
use sona_core::tempo::grid_phase;
use sona_core::unheard_pieces::{edge_cells, merge_unheard_pieces};
use sona_core::game::{cell_stage_notes, cells_covering, phase_of, stage_annotation_notes, CellModel, GameModel, GameParams, StageNotes, StageRuns};
use sona_core::{Annotation, GridMode, PYin, Producer, Transcriber};
use sona_eval::correction::edit_cost::{self, edit_cost, CellGrid, EditCost};
use sona_eval::correction::lyric_placement::{self, lyric_placement, LyricPlacement};
use sona_eval::correction::EditNote;
use sona_eval::holdout::authorize;
use sona_eval::quantize::grid_spacing_seconds;
use sona_eval::ust_corpus::{manifest_path, stems_dir, Manifest, SongUsts};
use sona_eval::ust::BlockKind;
use sona_eval::{ust, wav};
use serde_json::json;

type Notes = Vec<(f64, f64, i32)>;

/// Per landing: its title, and each song's edit cost and lyric placement.
type LandingRows = Vec<(String, Vec<(String, EditCost)>, Vec<(String, LyricPlacement)>)>;

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

/// The lead UST's sung notes in time order: onset, offset, MIDI pitch, lyric.
type Lyric = (f64, f64, f64, String);

/// A line placed in time by an aligner: its first UST note, start and end in seconds.
type TimedLine = (usize, f64, f64);

/// Output notes with the index (into the UST's sung notes) of the mora each carries.
type Placed = Vec<(f64, f64, i32, Option<usize>)>;

/// How the lyric lands on the notes (lyrics-first 01).
#[derive(Clone, Copy)]
enum Landing<'a> {
    /// One mora per note in order, from the first note that ends after the UST starts (the app today).
    InOrder,
    /// Runs of touching notes, each fitted with the UST moras whose midpoint lies inside (07 as ported).
    SonaPhrases,
    /// Groups of UST notes, each fitted onto the notes whose midpoint lies in its span; `rests`: the span's
    /// rests may hold moras.
    Groups { starts: &'a [usize], rests: bool },
    /// Lines placed in time by an aligner: (first UST note, start s, end s), fitted by `fit_lines`.
    Timed { lines: &'a [TimedLine] },
}

/// `notes` on the grid at `anchor` once `sung`'s moras land on them as `how` says.
fn land_lyric(notes: &Notes, sung: &[Lyric], how: Landing<'_>, spacing: f64, anchor: f64) -> Placed {
    let cell = |t: f64| ((t - anchor) / spacing).round() as i64;
    let time = |c: i64| anchor + c as f64 * spacing;
    let cells: Vec<CellNote> = notes.iter().map(|&(a, b, p)| CellNote { start: cell(a), end: cell(b), pitch: p }).collect();
    let kinds: Vec<Mora> = sung.iter().map(|(_, _, _, l)| mora_of(l)).collect();
    let mut placed: Vec<(CellNote, Option<usize>)> = Vec::with_capacity(cells.len());
    match how {
        Landing::InOrder => {
            let first_on = sung.first().map_or(f64::INFINITY, |s| s.0);
            let mut next = 0;
            for c in cells {
                let takes = time(c.end) > first_on + 0.5 * spacing && next < sung.len();
                placed.push((c, takes.then_some(next)));
                next += usize::from(takes);
            }
        }
        Landing::SonaPhrases => {
            let mut mids: Vec<(i64, usize)> = sung.iter().enumerate().map(|(k, &(a, b, _, _))| (cell(a) + cell(b), k)).collect();
            mids.sort();
            let mut i = 0;
            while i < cells.len() {
                let mut j = i + 1;
                while j < cells.len() && cells[j].start <= cells[j - 1].end {
                    j += 1;
                }
                let lo = mids.partition_point(|&(m, _)| m < 2 * cells[i].start);
                let hi = mids.partition_point(|&(m, _)| m < 2 * cells[j - 1].end);
                let ids: Vec<usize> = mids[lo..hi].iter().map(|&(_, k)| k).collect();
                let moras: Vec<Mora> = ids.iter().map(|&k| kinds[k]).collect();
                placed.extend(fit_phrase_moras(&cells[i..j], &moras).into_iter().map(|(n, m)| (n, m.map(|m| ids[m]))));
                i = j;
            }
        }
        Landing::Groups { starts, rests } => {
            let mut bounds: Vec<usize> = starts.iter().copied().filter(|&k| 0 < k && k < sung.len()).collect();
            bounds.extend([0, sung.len()]);
            bounds.sort_unstable();
            bounds.dedup();
            let groups: Vec<(usize, usize)> = bounds.windows(2).map(|w| (w[0], w[1])).collect();
            let mut members: Vec<Vec<CellNote>> = vec![Vec::new(); groups.len()];
            for c in cells {
                let mid = anchor + (c.start + c.end) as f64 / 2.0 * spacing;
                let k = groups.partition_point(|&(a, _)| sung[a].0 <= mid);
                match k.checked_sub(1) {
                    Some(k) if mid < sung[groups[k].1 - 1].1 => members[k].push(c),
                    _ => placed.push((c, None)),
                }
            }
            for (&(a, b), notes) in groups.iter().zip(&members) {
                let moras = &kinds[a..b];
                let fitted = if rests { fit_span_moras(notes, moras, (cell(sung[a].0), cell(sung[b - 1].1))) } else { fit_phrase_moras(notes, moras) };
                placed.extend(fitted.into_iter().map(|(n, m)| (n, m.map(|m| a + m))));
            }
            placed.sort_by_key(|(n, _)| (n.start, n.end));
        }
        Landing::Timed { lines } => {
            let spans: Vec<LineSpan> = lines.iter().map(|&(first, a, b)| LineSpan { first, start: (a - anchor) / spacing, end: (b - anchor) / spacing }).collect();
            placed = fit_lines(&cells, &kinds, &spans);
        }
    }
    placed.into_iter().map(|(n, m)| (time(n.start), time(n.end), n.pitch, m)).collect()
}

/// `v[key]`: a list of `[first UST note, start s, end s, …]` (any further fields, such as the aligner's
/// score, are ignored).
fn timed_lines(v: &serde_json::Value, key: &str) -> Result<Vec<TimedLine>, String> {
    let rows = v[key].as_array().ok_or(format!("no {key}"))?;
    rows.iter()
        .map(|r| match (r[0].as_u64(), r[1].as_f64(), r[2].as_f64()) {
            (Some(first), Some(a), Some(b)) if a <= b => Ok((first as usize, a, b)),
            _ => Err(format!("{key}: not [first, start, end] with start <= end: {r}")),
        })
        .collect()
}

/// The UST notes that begin a phrase: a run of notes touching on the UST's cells (anchor `offset`).
fn ust_phrase_starts(sung: &[Lyric], spacing: f64, offset: f64) -> Vec<usize> {
    let cell = |t: f64| ((t - offset) / spacing).round() as i64;
    let mut starts = Vec::new();
    let mut prev_end: Option<i64> = None;
    for (k, &(a, b, _, _)) in sung.iter().enumerate() {
        if prev_end.is_none_or(|e| cell(a) > e) {
            starts.push(k);
        }
        prev_end = Some(cell(b));
    }
    starts
}

/// `notes` (on the grid at `anchor`) with their one-cell pieces merged where `edges` has no cut.
fn merge_unheard(notes: &Notes, edges: &[i64], spacing: f64, anchor: f64) -> Notes {
    let cell = |t: f64| ((t - anchor) / spacing).round() as i64;
    let cells: Vec<CellNote> = notes.iter().map(|&(a, b, p)| CellNote { start: cell(a), end: cell(b), pitch: p }).collect();
    merge_unheard_pieces(&cells, edges).iter().map(|n| (anchor + n.start as f64 * spacing, anchor + n.end as f64 * spacing, n.pitch)).collect()
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
    let with_merge = args.iter().any(|a| a == "--merge-unheard-pieces");
    let mut costs: [Vec<(String, EditCost)>; 15] = Default::default();
    let with_trim = args.iter().any(|a| a == "--trim-releases");
    let with_ties = args.iter().any(|a| a == "--settle-near-ties");
    let with_placement = args.iter().any(|a| a == "--lyric-placement");
    let lyric_lines = flag("--lyric-lines").map(PathBuf::from);
    let line_times = flag("--lyric-line-times").map(PathBuf::from);
    let mut placements: LandingRows = Vec::new();
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
            let scored = cell_stage_notes(&model, &runs, notes, &f0, &rms, spacing, anchor, cells_covering(duration, spacing, anchor)).map_err(|e| e.to_string())?;
            let stage = scored.notes.clone();
            row[2 * j] = cells(&reference, &lead_only, spacing, offset);
            row[2 * j + 1] = cells(&reference, &stage, spacing, offset);
            if with_edit_cost && j == 1 {
                costs[1].push((song.id.clone(), edit_cost(&edit_notes(&reference), &edit_notes(&lead_only), &grid)));
                costs[2].push((song.id.clone(), edit_cost(&edit_notes(&reference), &edit_notes(&stage), &grid)));
                let sung: Vec<(f64, f64, Mora)> = usts
                    .lead
                    .iter()
                    .flat_map(|u| u.timeline().into_iter().filter(|p| p.kind == BlockKind::Note).map(move |p| (p.start_s + offset, p.end_s + offset, mora_of(&u.notes[p.i].lyric))))
                    .collect();
                if with_fit {
                    for (k, ns) in [(3, &lead_only), (4, &stage)] {
                        let fitted = fit_to_sung_phrases(ns, &sung, spacing, anchor);
                        costs[k].push((song.id.clone(), edit_cost(&edit_notes(&reference), &edit_notes(&fitted), &grid)));
                    }
                }
                if with_placement {
                    let mut lyrics: Vec<Lyric> = usts
                        .lead
                        .iter()
                        .flat_map(|u| u.timeline().into_iter().filter(|p| p.kind == BlockKind::Note).map(move |p| (p.start_s + offset, p.end_s + offset, f64::from(p.note_num), u.notes[p.i].lyric.clone())))
                        .collect();
                    lyrics.sort_by(|x, y| (x.0, x.1).partial_cmp(&(y.0, y.1)).expect("finite times"));
                    let truth: Vec<(EditNote, &str)> = lyrics.iter().map(|(a, b, p, l)| (EditNote { onset_s: *a, offset_s: *b, midi_pitch: *p }, l.as_str())).collect();
                    let phrase_starts = ust_phrase_starts(&lyrics, spacing, offset);
                    let lines: Option<(Vec<usize>, Vec<usize>)> = match &lyric_lines {
                        Some(dir) => {
                            let path = dir.join(format!("{}.json", song.id));
                            let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?).map_err(|e| format!("{}: {e}", path.display()))?;
                            let list = |k: &str| v[k].as_array().map(|a| a.iter().filter_map(|x| x.as_u64().map(|x| x as usize)).collect::<Vec<_>>()).ok_or(format!("{}: no {k}", path.display()));
                            Some((list("line_starts")?, list("space_starts")?))
                        }
                        None => None,
                    };
                    let timed: Option<(Vec<TimedLine>, Vec<TimedLine>)> = match &line_times {
                        Some(dir) => {
                            let path = dir.join(format!("{}.json", song.id));
                            let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?).map_err(|e| format!("{}: {e}", path.display()))?;
                            Some((timed_lines(&v, "lines").map_err(|e| format!("{}: {e}", path.display()))?, timed_lines(&v, "pieces").map_err(|e| format!("{}: {e}", path.display()))?))
                        }
                        None => None,
                    };
                    let whole = [0usize];
                    let mut landings: Vec<(&str, Landing<'_>)> = vec![
                        ("pasted in order, no fit (today)", Landing::InOrder),
                        ("fitted per run of touching notes (07 as ported)", Landing::SonaPhrases),
                        ("fitted per UST phrase", Landing::Groups { starts: &phrase_starts, rests: false }),
                        ("fitted per UST phrase, rests hold moras", Landing::Groups { starts: &phrase_starts, rests: true }),
                        ("fitted as one phrase", Landing::Groups { starts: &whole, rests: false }),
                        ("fitted as one phrase, rests hold moras", Landing::Groups { starts: &whole, rests: true }),
                    ];
                    let both: Vec<usize>;
                    if let Some((line_starts, space_starts)) = &lines {
                        both = line_starts.iter().chain(&phrase_starts).copied().collect();
                        landings.extend([
                            ("fitted per lyric line", Landing::Groups { starts: line_starts, rests: false }),
                            ("fitted per lyric line, rests hold moras", Landing::Groups { starts: line_starts, rests: true }),
                            ("fitted per lyric line and space", Landing::Groups { starts: space_starts, rests: false }),
                            ("fitted per lyric line and space, rests hold moras", Landing::Groups { starts: space_starts, rests: true }),
                            ("fitted per lyric line cut at UST phrases, rests hold moras", Landing::Groups { starts: &both, rests: true }),
                        ]);
                    }
                    if let Some((lines, pieces)) = &timed {
                        landings.extend([
                            ("fitted per aligned lyric line, rests hold moras", Landing::Timed { lines }),
                            ("fitted per aligned lyric line and space, rests hold moras", Landing::Timed { lines: pieces }),
                        ]);
                    }
                    for (out, ns) in [("GAME lead + octave rule", &lead_only), ("learned cell stage", &stage)] {
                        for (how, landing) in &landings {
                            let title = format!("{out}, lyric {how}");
                            let landed = land_lyric(ns, &lyrics, *landing, spacing, anchor);
                            let notes: Notes = landed.iter().map(|&(a, b, p, _)| (a, b, p)).collect();
                            let sung_notes: Vec<(EditNote, Option<&str>)> = landed.iter().map(|&(a, b, p, m)| (EditNote { onset_s: a, offset_s: b, midi_pitch: f64::from(p) }, m.map(|m| lyrics[m].3.as_str()))).collect();
                            let row = match placements.iter_mut().find(|(t, _, _)| *t == title) {
                                Some(row) => row,
                                None => {
                                    placements.push((title.clone(), Vec::new(), Vec::new()));
                                    placements.last_mut().expect("pushed")
                                }
                            };
                            row.1.push((song.id.clone(), edit_cost(&edit_notes(&reference), &edit_notes(&notes), &grid)));
                            row.2.push((song.id.clone(), lyric_placement(&truth, &sung_notes, &grid)));
                        }
                    }
                }
                if with_merge {
                    let edges = edge_cells(runs.vocals_notes.iter().filter(|n| n.voiced).map(|n| (n.onset_s, n.offset_s)), spacing, anchor);
                    for (k, ns) in [(5, &lead_only), (6, &stage)] {
                        let merged = merge_unheard(ns, &edges, spacing, anchor);
                        costs[k].push((song.id.clone(), edit_cost(&edit_notes(&reference), &edit_notes(&merged), &grid)));
                        if with_fit {
                            let fitted = fit_to_sung_phrases(&merged, &sung, spacing, anchor);
                            costs[k + 2].push((song.id.clone(), edit_cost(&edit_notes(&reference), &edit_notes(&fitted), &grid)));
                        }
                    }
                }
                if with_trim {
                    for (k, ns) in [(9, &lead_only), (10, &stage)] {
                        let trimmed = trim_releases(ns, &rms, spacing, ReleaseRule::default());
                        costs[k].push((song.id.clone(), edit_cost(&edit_notes(&reference), &edit_notes(&trimmed), &grid)));
                    }
                }
                if with_ties {
                    let settle = |ns: &Notes| settle_near_ties(ns, &runs.lead_notes, &runs.vocals_notes, spacing, anchor, NearTieRule::default());
                    for (k, ns) in [(11, &lead_only), (12, &stage)] {
                        costs[k].push((song.id.clone(), edit_cost(&edit_notes(&reference), &edit_notes(&settle(ns)), &grid)));
                    }
                    if with_trim {
                        let both = trim_releases(&settle(&lead_only), &rms, spacing, ReleaseRule::default());
                        costs[13].push((song.id.clone(), edit_cost(&edit_notes(&reference), &edit_notes(&both), &grid)));
                    }
                    if with_merge {
                        let edges = edge_cells(runs.vocals_notes.iter().filter(|n| n.voiced).map(|n| (n.onset_s, n.offset_s)), spacing, anchor);
                        let both = merge_unheard(&settle(&stage), &edges, spacing, anchor);
                        costs[14].push((song.id.clone(), edit_cost(&edit_notes(&reference), &edit_notes(&both), &grid)));
                    }
                }
            }
            if let (Some(dir), 1) = (&dump, j) {
                let outputs = [("pyin", &sona), ("lead", &lead_only), ("stage", &stage)];
                let costs: Vec<_> = outputs.iter().map(|(name, ns)| (*name, edit_cost(&edit_notes(&reference), &edit_notes(ns), &grid))).collect();
                let inputs = StageInputs { spacing, anchor, lead_notes: &runs.lead_notes, lead_ratios: &runs.lead_ratios, vocals_notes: &runs.vocals_notes, sona_notes: notes, lead_frames: &runs.lead_frames, vocals_frames: &runs.vocals_frames, f0: &f0, rms: &rms };
                let song_dump = dump_song(&model, &inputs, cells_covering(duration, spacing, anchor), &scored, &lead, sr, &usts, offset)?;
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
            "GAME lead + octave rule, unheard pieces merged (opt-in)",
            "learned cell stage, unheard pieces merged (opt-in)",
            "GAME lead + octave rule, unheard pieces merged, each phrase's lyric fitted (opt-in)",
            "learned cell stage, unheard pieces merged, each phrase's lyric fitted (opt-in)",
            "GAME lead + octave rule, releases trimmed (opt-in)",
            "learned cell stage, releases trimmed (opt-in; not applied: raises its cost)",
            "GAME lead + octave rule, near ties settled (opt-in)",
            "learned cell stage, near ties settled (opt-in)",
            "GAME lead + octave rule, near ties settled, releases trimmed (opt-in)",
            "learned cell stage, near ties settled, unheard pieces merged (opt-in)",
        ];
        for (title, rows) in titles.iter().zip(&costs).filter(|(_, rows)| !rows.is_empty()) {
            print!("{}", edit_cost::report(title, rows));
        }
        for (title, costs, placed) in &placements {
            print!("{}", edit_cost::report(title, costs));
            print!("{}", lyric_placement::report(title, placed));
        }
    }
    Ok(())
}

/// The stage's cells and notes in detail, for `--dump`: candidates, sources, class probabilities and
/// each shipped note's confidence and sub-scores (as `sona transcribe` gives them, on the lead's energy).
#[allow(clippy::too_many_arguments)]
fn dump_song(model: &CellModel, inputs: &StageInputs<'_>, (k0, n): (i64, usize), scored: &StageNotes, lead: &[f32], sr: u32, usts: &SongUsts, offset: f64) -> Result<serde_json::Value, String> {
    let stage = &scored.notes;
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
    let envelope = Transcriber::default().onset.detect(lead, sr).energy_envelope;
    let shipped = stage_annotation_notes(scored, &Transcriber::default().scorer, &envelope);
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
        "confidence": shipped.iter().map(|n| (n.onset_s, n.offset_s, n.midi_pitch, n.confidence, [n.sub_scores.pitch_stability, n.sub_scores.voicing, n.sub_scores.onset_clarity, n.sub_scores.offset_clarity])).collect::<Vec<_>>(),
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
