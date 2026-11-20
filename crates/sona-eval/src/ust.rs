//! **UST parser** — UTAU's sequence format, the reference side of the in-domain
//! UST corpus gate (`.scratch/ust-corpus-gate/`).
//!
//! A UST is a *contiguous, tick-space* note list: every block starts exactly
//! where the previous one ended, and silence is an explicit block whose lyric is
//! `R`. That makes a UST the **ideal MIDI** a skilled transcriber chose — not a
//! performance annotation — which is why it can be scored quantized-vs-quantized.
//!
//! Only the *musical* fields are kept:
//! - `[#SETTING]`: `Tempo` (BPM) and `ProjectName`.
//! - `[#NNNN]` note blocks: `Length` (ticks @ [`TICKS_PER_QUARTER`]), `Lyric`,
//!   `NoteNum` (a MIDI note number; 60 = C4), and an optional note-level `Tempo`
//!   (a tempo change taking effect at that note).
//!
//! Synthesis-only parameters (`PreUtterance`, `VoiceOverlap`, `Intensity`,
//! `Modulation`, `Envelope`, `PBS`/`PBW`/`PBY`/`PBM`, `Flags`, …) are ignored:
//! they move *voicebank sample placement*, not the musical note position.
//!
//! **Encoding is decided explicitly, never assumed** (see [`decode`]): USTs are
//! commonly Shift-JIS (CP932) with kana lyrics. The chosen encoding is recorded
//! on the parsed [`Ust`] so an audit can show it.
//!
//! The corpus itself (Pifuyuu's USTs) is never committed: the author's terms
//! forbid redistribution, including converted forms. It lives in a git-ignored
//! cache ([`corpus_dir`]); the committed `fixtures/ust/*.ust` are synthetic,
//! hand-authored files in the same format.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use encoding_rs::{Encoding, SHIFT_JIS, UTF_8};

use crate::error::EvalError;

/// UST tick resolution: ticks per quarter note. Fixed by the format.
pub const TICKS_PER_QUARTER: u32 = 480;

/// The `SONA_UST_DIR` override; when unset the UST cache is expected under the
/// crate's git-ignored `data/pifuyuu-ust` (where the fetch script will put it).
pub const CORPUS_ENV: &str = "SONA_UST_DIR";

/// Resolve where the UST cache should live: `$SONA_UST_DIR` if set, else the
/// crate-local `data/pifuyuu-ust`. Existence is checked by [`discover`].
pub fn corpus_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os(CORPUS_ENV) {
        return PathBuf::from(dir);
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data/pifuyuu-ust")
}

/// The committed synthetic fixture USTs (`crates/sona-eval/fixtures/ust`).
pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/ust")
}

/// Every `*.ust` file under `root` (recursive), sorted by path. Fails loudly
/// with [`EvalError::UstCorpusMissing`] if `root` is absent or holds no USTs —
/// a silent empty audit would read as a clean pass.
pub fn discover(root: &Path) -> Result<Vec<PathBuf>, EvalError> {
    if !root.is_dir() {
        return Err(EvalError::UstCorpusMissing(root.to_path_buf()));
    }
    let mut out = Vec::new();
    collect_usts(root, &mut out)?;
    if out.is_empty() {
        return Err(EvalError::UstCorpusMissing(root.to_path_buf()));
    }
    out.sort();
    Ok(out)
}

/// The text encoding a UST was decoded with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UstEncoding {
    /// UTF-8 — declared (`Charset=UTF-8` / BOM) or detected by strict validation.
    Utf8,
    /// Shift-JIS as UTAU writes it on Japanese Windows (CP932 / Windows-31J).
    ShiftJis,
}

impl UstEncoding {
    fn encoding(self) -> &'static Encoding {
        match self {
            UstEncoding::Utf8 => UTF_8,
            UstEncoding::ShiftJis => SHIFT_JIS,
        }
    }

    /// Short label for reports.
    pub fn label(self) -> &'static str {
        match self {
            UstEncoding::Utf8 => "UTF-8",
            UstEncoding::ShiftJis => "Shift-JIS",
        }
    }
}

/// What a note block represents musically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// A sung note.
    Note,
    /// Silence (lyric `R` / `r`).
    Rest,
    /// A breath sample (`息`, `吸`, `br`, `breath`, …). Not a pitched note sona
    /// would transcribe, and — like a rest — it marks the end of the sung note
    /// before it.
    Breath,
}

/// One `[#NNNN]` block, musical fields only.
#[derive(Debug, Clone, PartialEq)]
pub struct UstNote {
    /// The block number (`NNNN`).
    pub index: u32,
    /// Duration in ticks ([`TICKS_PER_QUARTER`] per quarter note).
    pub length: u32,
    /// The lyric, verbatim (kana, romaji, VCV `a ら`, `R` for rests, …).
    pub lyric: String,
    /// MIDI note number (60 = C4). Present on rests too, where it is meaningless.
    pub note_num: u8,
    /// A note-level tempo change (BPM) taking effect at the start of this block.
    pub tempo: Option<f64>,
}

impl UstNote {
    /// Classify the block from its lyric.
    pub fn kind(&self) -> BlockKind {
        let lyric = self.lyric.trim();
        if lyric.eq_ignore_ascii_case("r") {
            return BlockKind::Rest;
        }
        let lower = lyric.to_ascii_lowercase();
        let is_breath = matches!(lyric, "息" | "吸" | "吸う")
            || lower == "breath"
            // `br`, `br1`, `br2`… — numbered breath samples.
            || lower.strip_prefix("br").is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit()));
        if is_breath {
            BlockKind::Breath
        } else {
            BlockKind::Note
        }
    }
}

/// A parsed UST.
#[derive(Debug, Clone, PartialEq)]
pub struct Ust {
    /// The encoding the file was decoded with.
    pub encoding: UstEncoding,
    /// `[#SETTING] ProjectName`, if present.
    pub project_name: Option<String>,
    /// The initial tempo (BPM): `[#SETTING] Tempo`, or the first block's
    /// note-level `Tempo` when the setting is absent (some editors write it there).
    pub tempo: f64,
    /// Every note block — notes, rests and breaths — in sequence order.
    pub notes: Vec<UstNote>,
}

/// A block placed on the absolute timeline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    /// Index into [`Ust::notes`].
    pub i: usize,
    /// What the block is.
    pub kind: BlockKind,
    /// Absolute start tick from the top of the sequence.
    pub start_tick: u64,
    /// Absolute end tick (exclusive).
    pub end_tick: u64,
    /// Start, in seconds (tempo changes applied).
    pub start_s: f64,
    /// End, in seconds.
    pub end_s: f64,
    /// MIDI note number.
    pub note_num: u8,
}

impl Placed {
    /// Duration in seconds.
    pub fn duration_s(&self) -> f64 {
        self.end_s - self.start_s
    }
}

impl Ust {
    /// Read and parse a UST file.
    pub fn load(path: &Path) -> Result<Ust, EvalError> {
        let bytes = fs::read(path).map_err(|e| EvalError::io(path, e))?;
        Ust::parse_bytes(&bytes, path)
    }

    /// Parse raw UST bytes, deciding the encoding explicitly (see [`decode`]).
    /// `path` is only used to locate errors.
    pub fn parse_bytes(bytes: &[u8], path: &Path) -> Result<Ust, EvalError> {
        let (text, encoding) = decode(bytes, path)?;
        Ust::parse_str(&text, encoding, path)
    }

    /// Parse already-decoded UST text.
    pub fn parse_str(text: &str, encoding: UstEncoding, path: &Path) -> Result<Ust, EvalError> {
        let mut section = Section::None;
        let mut setting_tempo: Option<f64> = None;
        let mut project_name: Option<String> = None;
        let mut notes: Vec<UstNote> = Vec::new();
        let mut pending: Option<PendingNote> = None;

        for (line_no, raw) in text.lines().enumerate() {
            let line = raw.trim_end_matches('\r');
            let loc = |reason: String| EvalError::parse(path, format!("line {}: {reason}", line_no + 1));

            if let Some(name) = line.strip_prefix("[#").and_then(|s| s.strip_suffix(']')) {
                if let Some(p) = pending.take() {
                    notes.push(p.finish(path)?);
                }
                section = match name {
                    "SETTING" => Section::Setting,
                    _ if !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit()) => {
                        let index = name.parse::<u32>().map_err(|_| loc(format!("bad block number '{name}'")))?;
                        pending = Some(PendingNote::new(index, line_no + 1));
                        Section::Note
                    }
                    // VERSION, TRACKEND, and plugin-temp sections (PREV, NEXT,
                    // INSERT, DELETE) carry no musical content we keep.
                    _ => Section::None,
                };
                continue;
            }

            let Some((key, value)) = line.split_once('=') else { continue };
            match section {
                Section::Setting => match key {
                    "Tempo" => setting_tempo = Some(parse_tempo(value).map_err(loc)?),
                    "ProjectName" => project_name = Some(value.to_string()),
                    _ => {}
                },
                Section::Note => {
                    let p = pending.as_mut().expect("Section::Note always has a pending block");
                    match key {
                        "Length" => {
                            p.length = Some(value.trim().parse::<u32>().map_err(|_| {
                                loc(format!("expected an integer tick Length, got '{value}'"))
                            })?)
                        }
                        "Lyric" => p.lyric = Some(value.to_string()),
                        "NoteNum" => {
                            let n = value.trim().parse::<u8>().ok().filter(|&n| n <= 127);
                            p.note_num = Some(n.ok_or_else(|| loc(format!("NoteNum must be 0–127, got '{value}'")))?);
                        }
                        "Tempo" => p.tempo = Some(parse_tempo(value).map_err(loc)?),
                        _ => {}
                    }
                }
                Section::None => {}
            }
        }
        if let Some(p) = pending.take() {
            notes.push(p.finish(path)?);
        }

        let tempo = setting_tempo
            .or_else(|| notes.first().and_then(|n| n.tempo))
            .ok_or_else(|| EvalError::parse(path, "no Tempo in [#SETTING] or on the first note"))?;

        Ok(Ust { encoding, project_name, tempo, notes })
    }

    /// Serialize the kept (musical) fields back to UST text. Synthesis
    /// parameters are not preserved — this is the parser's round-trip check
    /// (`parse(write(u)) == u`), not a lossless editor.
    pub fn to_ust_string(&self) -> String {
        let mut s = String::new();
        s.push_str("[#VERSION]\r\nUST Version1.2\r\n");
        if self.encoding == UstEncoding::Utf8 {
            s.push_str("Charset=UTF-8\r\n");
        }
        s.push_str("[#SETTING]\r\n");
        let _ = write!(s, "Tempo={}\r\n", self.tempo);
        if let Some(name) = &self.project_name {
            let _ = write!(s, "ProjectName={name}\r\n");
        }
        for n in &self.notes {
            let _ = write!(s, "[#{:04}]\r\nLength={}\r\nLyric={}\r\nNoteNum={}\r\n", n.index, n.length, n.lyric, n.note_num);
            if let Some(t) = n.tempo {
                let _ = write!(s, "Tempo={t}\r\n");
            }
        }
        s.push_str("[#TRACKEND]\r\n");
        s
    }

    /// [`to_ust_string`](Self::to_ust_string), encoded back into this UST's own
    /// encoding — so a Shift-JIS file round-trips as Shift-JIS bytes. Fails if a
    /// lyric cannot be represented (e.g. `ゔ` has no CP932 code point).
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let text = self.to_ust_string();
        let (bytes, _, unmappable) = self.encoding.encoding().encode(&text);
        if unmappable {
            return Err(format!("text not representable in {}", self.encoding.label()));
        }
        Ok(bytes.into_owned())
    }

    /// Place every block on the absolute timeline, applying note-level tempo
    /// changes from the block that carries them onward.
    pub fn timeline(&self) -> Vec<Placed> {
        let mut out = Vec::with_capacity(self.notes.len());
        let mut tempo = self.tempo;
        let mut tick: u64 = 0;
        let mut t_s = 0.0f64;
        for (i, n) in self.notes.iter().enumerate() {
            if let Some(change) = n.tempo {
                tempo = change;
            }
            let dur_s = ticks_to_seconds(n.length as f64, tempo);
            out.push(Placed {
                i,
                kind: n.kind(),
                start_tick: tick,
                end_tick: tick + n.length as u64,
                start_s: t_s,
                end_s: t_s + dur_s,
                note_num: n.note_num,
            });
            tick += n.length as u64;
            t_s += dur_s;
        }
        out
    }

    /// Effective tempo changes after the initial tempo, as `(block index, bpm)`.
    /// A note-level `Tempo` equal to the tempo already in force is not a change.
    pub fn tempo_changes(&self) -> Vec<(usize, f64)> {
        let mut tempo = self.tempo;
        let mut out = Vec::new();
        for (i, n) in self.notes.iter().enumerate() {
            if let Some(t) = n.tempo {
                if (t - tempo).abs() > 1e-9 {
                    out.push((i, t));
                }
                tempo = t;
            }
        }
        out
    }
}

/// Convert a tick count to seconds at `bpm`.
pub fn ticks_to_seconds(ticks: f64, bpm: f64) -> f64 {
    ticks * 60.0 / (bpm * TICKS_PER_QUARTER as f64)
}

/// Decode UST bytes. The encoding is decided **explicitly**, in this order:
///
/// 1. a UTF-8 **BOM**;
/// 2. a **`Charset=`** declaration (scanned as ASCII, which reads identically in
///    Shift-JIS and UTF-8) — honoured for UTF-8 and Shift-JIS labels;
/// 3. **strict UTF-8 validation**: Shift-JIS kana bytes are effectively never
///    valid UTF-8, whereas a UTF-8 file decoded as Shift-JIS silently becomes
///    mojibake *without* decode errors — so validation must come first;
/// 4. otherwise **Shift-JIS (CP932)**, UTAU's native encoding — and malformed
///    Shift-JIS is a located error, never silently replaced.
pub fn decode(bytes: &[u8], path: &Path) -> Result<(String, UstEncoding), EvalError> {
    if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        let text = std::str::from_utf8(rest)
            .map_err(|e| EvalError::parse(path, format!("UTF-8 BOM but invalid UTF-8: {e}")))?;
        return Ok((text.to_string(), UstEncoding::Utf8));
    }

    let declared = declared_charset(bytes);
    let encoding = match declared.as_deref() {
        Some(label) => match Encoding::for_label(label.as_bytes()) {
            Some(e) if e == UTF_8 => UstEncoding::Utf8,
            Some(e) if e == SHIFT_JIS => UstEncoding::ShiftJis,
            _ => return Err(EvalError::parse(path, format!("unsupported Charset '{label}'"))),
        },
        None if std::str::from_utf8(bytes).is_ok() => UstEncoding::Utf8,
        None => UstEncoding::ShiftJis,
    };

    let (text, had_errors) = encoding.encoding().decode_without_bom_handling(bytes);
    if had_errors {
        return Err(EvalError::parse(path, format!("malformed {} text", encoding.label())));
    }
    Ok((text.into_owned(), encoding))
}

/// The value of the first `Charset=` line, read byte-wise as ASCII.
fn declared_charset(bytes: &[u8]) -> Option<String> {
    bytes.split(|&b| b == b'\n').find_map(|line| {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let value = line.strip_prefix(b"Charset=")?;
        let value = std::str::from_utf8(value).ok()?.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn parse_tempo(value: &str) -> Result<f64, String> {
    // Some locales write a decimal comma (`Tempo=150,00`).
    let t = value.trim().replace(',', ".").parse::<f64>().map_err(|_| format!("bad Tempo '{value}'"))?;
    if t.is_finite() && t > 0.0 {
        Ok(t)
    } else {
        Err(format!("Tempo must be positive, got '{value}'"))
    }
}

#[derive(Clone, Copy)]
enum Section {
    None,
    Setting,
    Note,
}

/// A note block being accumulated until its section ends.
struct PendingNote {
    index: u32,
    header_line: usize,
    length: Option<u32>,
    lyric: Option<String>,
    note_num: Option<u8>,
    tempo: Option<f64>,
}

impl PendingNote {
    fn new(index: u32, header_line: usize) -> Self {
        PendingNote { index, header_line, length: None, lyric: None, note_num: None, tempo: None }
    }

    fn finish(self, path: &Path) -> Result<UstNote, EvalError> {
        let missing = |field: &str| {
            EvalError::parse(path, format!("line {}: block [#{:04}] has no {field}", self.header_line, self.index))
        };
        Ok(UstNote {
            index: self.index,
            length: self.length.ok_or_else(|| missing("Length"))?,
            lyric: self.lyric.ok_or_else(|| missing("Lyric"))?,
            note_num: self.note_num.ok_or_else(|| missing("NoteNum"))?,
            tempo: self.tempo,
        })
    }
}

fn collect_usts(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), EvalError> {
    for entry in fs::read_dir(dir).map_err(|e| EvalError::io(dir, e))? {
        let path = entry.map_err(|e| EvalError::io(dir, e))?.path();
        if path.is_dir() {
            collect_usts(&path, out)?;
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ust")) {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Ust {
        Ust::load(&fixtures_dir().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
    }

    fn lyrics(u: &Ust) -> Vec<&str> {
        u.notes.iter().map(|n| n.lyric.as_str()).collect()
    }

    #[test]
    fn staccato_decodes_shift_jis_kana() {
        let u = fixture("staccato.ust");
        assert_eq!(u.encoding, UstEncoding::ShiftJis);
        assert_eq!(u.tempo, 150.0);
        assert_eq!(u.project_name.as_deref(), Some("さくらちる"));
        assert_eq!(lyrics(&u), ["R", "さ", "R", "く", "R", "ら", "R", "ち", "R", "る", "R"]);
        assert_eq!(u.notes[1].note_num, 72);
        assert_eq!(u.notes[1].length, 240);
        // Synthesis params (PBS, Envelope, Flags, …) are ignored, not mis-read.
        assert!(u.notes.iter().all(|n| n.tempo.is_none()));
    }

    #[test]
    fn legato_has_breath_and_triplets() {
        let u = fixture("legato.ust");
        assert_eq!(u.encoding, UstEncoding::ShiftJis);
        assert_eq!(lyrics(&u), ["R", "あ", "い", "う", "え", "お", "か", "息", "き", "R"]);
        assert_eq!(u.notes[7].kind(), BlockKind::Breath);
        assert_eq!(u.notes[2].length, 160);
    }

    #[test]
    fn utf8_with_charset_declaration_decodes() {
        let u = fixture("utf8.ust");
        assert_eq!(u.encoding, UstEncoding::Utf8);
        // ゔ has no CP932 code point: it only survives an explicit UTF-8 decode.
        assert_eq!(lyrics(&u), ["R", "ゔ", "a ら", "- ん", "R"]);
        assert_eq!(u.project_name.as_deref(), Some("ゔぁいおりん"));
        assert_eq!(u.tempo, 140.0);
    }

    #[test]
    fn tempo_change_and_lowercase_rest() {
        let u = fixture("tempo-change.ust");
        assert_eq!(u.notes[0].kind(), BlockKind::Rest, "lowercase r is a rest");
        assert_eq!(u.tempo, 128.0);
        assert_eq!(u.tempo_changes(), vec![(2, 64.0)]);

        // 480 @128 = 0.46875 s; the change to 64 BPM doubles block 2 onward.
        let t = u.timeline();
        let ends: Vec<f64> = t.iter().map(|p| p.end_s).collect();
        let want = [0.46875, 0.9375, 1.40625, 1.875, 2.8125];
        for (got, want) in ends.iter().zip(want) {
            assert!((got - want).abs() < 1e-9, "{ends:?}");
        }
        assert_eq!(t.last().unwrap().end_tick, 1920);
    }

    #[test]
    fn every_fixture_round_trips_through_text_and_bytes() {
        let paths = discover(&fixtures_dir()).unwrap();
        assert!(paths.len() >= 3, "expected the committed fixtures, got {paths:?}");
        for path in paths {
            let u = Ust::load(&path).unwrap();
            let again = Ust::parse_str(&u.to_ust_string(), u.encoding, &path).unwrap();
            assert_eq!(u, again, "text round-trip: {}", path.display());
            let bytes = u.to_bytes().unwrap();
            assert_eq!(u, Ust::parse_bytes(&bytes, &path).unwrap(), "byte round-trip: {}", path.display());
        }
    }

    #[test]
    fn shift_jis_fixture_is_not_mistaken_for_utf8() {
        // Guard: the committed file really is Shift-JIS bytes, not UTF-8.
        let bytes = fs::read(fixtures_dir().join("staccato.ust")).unwrap();
        assert!(std::str::from_utf8(&bytes).is_err());
    }

    #[test]
    fn malformed_shift_jis_is_an_error_not_mojibake() {
        // 0x82 is a CP932 lead byte; 0x20 is not a valid trail byte.
        let bytes = b"[#SETTING]\nTempo=120\n[#0000]\nLength=480\nLyric=\x82\x20\nNoteNum=60\n";
        let err = Ust::parse_bytes(bytes, Path::new("bad.ust")).unwrap_err();
        assert!(err.to_string().contains("Shift-JIS"), "{err}");
    }

    #[test]
    fn declared_charset_wins_over_detection() {
        // ASCII-only content is valid UTF-8, but the declaration says Shift-JIS.
        let text = b"[#VERSION]\r\nCharset=shift_jis\r\n[#SETTING]\r\nTempo=120\r\n";
        let u = Ust::parse_bytes(text, Path::new("x.ust")).unwrap();
        assert_eq!(u.encoding, UstEncoding::ShiftJis);
    }

    #[test]
    fn tempo_falls_back_to_first_note() {
        let text = "[#SETTING]\n[#0000]\nLength=480\nLyric=R\nNoteNum=60\nTempo=175\n";
        let u = Ust::parse_str(text, UstEncoding::Utf8, Path::new("x.ust")).unwrap();
        assert_eq!(u.tempo, 175.0);
        assert!(u.tempo_changes().is_empty(), "initial tempo is not a change");
    }

    #[test]
    fn missing_tempo_and_missing_fields_are_located_errors() {
        let no_tempo = "[#SETTING]\n[#0000]\nLength=480\nLyric=R\nNoteNum=60\n";
        assert!(Ust::parse_str(no_tempo, UstEncoding::Utf8, Path::new("x.ust")).is_err());

        let no_len = "[#SETTING]\nTempo=120\n[#0000]\nLyric=あ\nNoteNum=60\n[#TRACKEND]\n";
        let err = Ust::parse_str(no_len, UstEncoding::Utf8, Path::new("x.ust")).unwrap_err();
        assert!(err.to_string().contains("[#0000] has no Length"), "{err}");

        let bad_num = "[#SETTING]\nTempo=120\n[#0000]\nLength=480\nLyric=あ\nNoteNum=200\n";
        assert!(Ust::parse_str(bad_num, UstEncoding::Utf8, Path::new("x.ust")).is_err());
    }

    #[test]
    fn classifies_breaths_and_rests() {
        let kind = |lyric: &str| UstNote { index: 0, length: 1, lyric: lyric.into(), note_num: 60, tempo: None }.kind();
        assert_eq!(kind("R"), BlockKind::Rest);
        assert_eq!(kind(" r "), BlockKind::Rest);
        for b in ["息", "吸", "br", "br2", "Breath"] {
            assert_eq!(kind(b), BlockKind::Breath, "{b}");
        }
        for n in ["ら", "a ら", "bra", "ra", "-"] {
            assert_eq!(kind(n), BlockKind::Note, "{n}");
        }
    }

    #[test]
    fn decimal_comma_tempo_parses() {
        assert_eq!(parse_tempo("150,00"), Ok(150.0));
        assert!(parse_tempo("0").is_err());
    }

    #[test]
    fn discover_missing_root_fails_loudly() {
        let err = discover(Path::new("/no/such/ust/dir")).unwrap_err();
        assert!(matches!(err, EvalError::UstCorpusMissing(_)));
    }
}
