use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::stems::{StemCache, StemKey};

#[derive(Debug, Clone)]
pub struct Tools {
    pub demucs_python: PathBuf,
    pub separator_bin: PathBuf,
    pub separator_models: PathBuf,
    pub lead_model: String,
    pub ffmpeg: PathBuf,
    /// The htdemucs weights in torch's cache, where demucs downloads them on its first run.
    pub demucs_weights: PathBuf,
    pub game_dir: Option<PathBuf>,
    pub cell_model: Option<PathBuf>,
}

/// Where jobs and cached stems live, how many of each to keep, and how long a tool may run (ADR-0030).
#[derive(Debug, Clone)]
pub struct Settings {
    pub jobs_dir: PathBuf,
    pub stems_dir: PathBuf,
    /// Finished jobs kept on disk, newest first; at least 1.
    pub keep_jobs: usize,
    /// Stem-cache entries kept, most recently used first; 0 turns the cache off.
    pub keep_stems: usize,
    pub tool_timeout: ToolTimeout,
}

/// A separator or ffmpeg run is stopped after `min_s` seconds, or `per_audio_s` seconds per second of
/// audio, whichever is longer.
#[derive(Debug, Clone, Copy)]
pub struct ToolTimeout {
    pub min_s: f64,
    pub per_audio_s: f64,
}

impl ToolTimeout {
    fn for_audio(self, audio_s: f64) -> Duration {
        Duration::from_secs_f64(self.min_s.max(self.per_audio_s * audio_s).clamp(0.0, 1e7))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Separation {
    None,
    Vocals,
    Lead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Pyin,
    Game,
    Cells,
}

#[derive(Debug, Clone)]
pub struct JobSpec {
    pub name: String,
    pub bpm: f64,
    pub grid: u32,
    pub separation: Separation,
    pub engine: Engine,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    Queued,
    Decoding,
    SeparatingVocals,
    SeparatingLead,
    Transcribing,
    Done,
    Failed(String),
    Cancelled,
}

impl Phase {
    pub fn label(&self) -> &'static str {
        match self {
            Phase::Queued => "queued",
            Phase::Decoding => "decoding",
            Phase::SeparatingVocals => "separating-vocals",
            Phase::SeparatingLead => "separating-lead",
            Phase::Transcribing => "transcribing",
            Phase::Done => "done",
            Phase::Failed(_) => "failed",
            Phase::Cancelled => "cancelled",
        }
    }

    pub fn is_final(&self) -> bool {
        matches!(self, Phase::Done | Phase::Failed(_) | Phase::Cancelled)
    }
}

pub struct Job {
    pub id: String,
    pub spec: JobSpec,
    pub dir: PathBuf,
    pub created: Instant,
    phase: Mutex<Phase>,
    cancel: AtomicBool,
    child: Mutex<Option<Child>>,
    /// The external tool running now, by its log's name.
    tool: Mutex<Option<&'static str>>,
    /// The upload's length, which sets each tool's time limit.
    audio_s: Mutex<f64>,
    /// When each step began, in order (ADR-0024).
    steps: Mutex<Vec<(&'static str, Instant)>>,
    stems: Mutex<Option<sona_core::StemSource>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Job {
    pub fn phase(&self) -> Phase {
        lock(&self.phase).clone()
    }

    fn set_phase(&self, next: Phase) -> bool {
        let mut p = lock(&self.phase);
        if p.is_final() {
            return false;
        }
        *p = next;
        true
    }

    pub fn cancel(&self) -> bool {
        self.cancel.store(true, Ordering::SeqCst);
        if let Some(child) = lock(&self.child).as_mut() {
            let _ = child.kill();
        }
        self.set_phase(Phase::Cancelled)
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    pub fn vocals_path(&self) -> PathBuf {
        self.dir.join("vocals.wav")
    }

    pub fn provenance_path(&self) -> PathBuf {
        self.dir.join("job.json")
    }

    pub fn annotation_path(&self) -> PathBuf {
        self.dir.join("annotation.json")
    }

    pub fn stem_path(&self) -> PathBuf {
        self.dir.join("stem.wav")
    }

    pub fn log_tail(&self, lines: usize) -> Vec<String> {
        let text = fs::read_to_string(self.dir.join("job.log")).unwrap_or_default();
        let all: Vec<&str> = text.lines().collect();
        all[all.len().saturating_sub(lines)..].iter().map(|s| s.to_string()).collect()
    }

    /// The last line the running tool printed (demucs's progress bar, say), while one runs.
    pub fn tool_line(&self) -> Option<String> {
        use std::io::{Read, Seek, SeekFrom};
        let label = (*lock(&self.tool))?;
        let mut f = fs::File::open(self.dir.join(format!("{label}.log"))).ok()?;
        let len = f.metadata().ok()?.len();
        f.seek(SeekFrom::Start(len.saturating_sub(4096))).ok()?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).ok()?;
        let text = String::from_utf8_lossy(&buf);
        let line = text.split(['\r', '\n']).map(str::trim).filter(|l| !l.is_empty()).next_back()?;
        Some(format!("{label}: {}", line.chars().take(160).collect::<String>()))
    }

    fn log(&self, line: &str) {
        use std::io::Write;
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(self.dir.join("job.log")) {
            let _ = writeln!(f, "[{:6.1}s] {line}", self.created.elapsed().as_secs_f64());
        }
    }
}

type JobMap = Arc<Mutex<HashMap<String, Arc<Job>>>>;

pub struct Queue {
    jobs: JobMap,
    tx: Mutex<Sender<Arc<Job>>>,
    root: PathBuf,
    counter: AtomicU64,
}

struct Worker {
    tools: Tools,
    settings: Settings,
    cache: StemCache,
    jobs: JobMap,
}

impl Queue {
    pub fn start(settings: Settings, tools: Tools) -> std::io::Result<Arc<Queue>> {
        fs::create_dir_all(&settings.jobs_dir)?;
        install_panic_hook();
        probe_tool_versions(tools.clone());
        let (tx, rx) = channel::<Arc<Job>>();
        let jobs = JobMap::default();
        let root = settings.jobs_dir.clone();
        let worker = Worker { cache: StemCache::new(settings.stems_dir.clone(), settings.keep_stems), tools, settings, jobs: jobs.clone() };
        std::thread::spawn(move || {
            worker.tidy();
            for job in rx {
                if catch_unwind(AssertUnwindSafe(|| run(&job, &worker))).is_err() {
                    let _ = catch_unwind(AssertUnwindSafe(|| crashed(&job, &worker)));
                }
                worker.tidy();
            }
        });
        Ok(Arc::new(Queue { jobs, tx: Mutex::new(tx), root, counter: AtomicU64::new(0) }))
    }

    pub fn submit(&self, spec: JobSpec, input_ext: &str, audio: &[u8]) -> std::io::Result<Arc<Job>> {
        let job = {
            let mut jobs = lock(&self.jobs);
            let n = self.counter.fetch_add(1, Ordering::SeqCst);
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
            let id = format!("{stamp:x}{n:04x}");
            let dir = self.root.join(&id);
            fs::create_dir_all(&dir)?;
            fs::write(dir.join(format!("input.{input_ext}")), audio)?;
            let job = Arc::new(Job {
                id: id.clone(),
                spec,
                dir,
                created: Instant::now(),
                phase: Mutex::new(Phase::Queued),
                cancel: AtomicBool::new(false),
                child: Mutex::new(None),
                tool: Mutex::new(None),
                audio_s: Mutex::new(0.0),
                steps: Mutex::new(vec![(Phase::Queued.label(), Instant::now())]),
                stems: Mutex::new(None),
            });
            job.log(&format!("received {} bytes", audio.len()));
            jobs.insert(id, job.clone());
            job
        };
        lock(&self.tx).send(job.clone()).map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(job)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn get(&self, id: &str) -> Option<Arc<Job>> {
        lock(&self.jobs).get(id).cloned()
    }
}

fn is_job_id(name: &str) -> bool {
    !name.is_empty() && name.len() <= 32 && name.bytes().all(|b| b.is_ascii_hexdigit())
}

impl Worker {
    /// Between jobs, so no job is running: delete all but the `keep_jobs` newest finished jobs (queued
    /// ones are never touched, nor folders that aren't jobs), and evict the stem cache to its limit.
    fn tidy(&self) {
        let _ = catch_unwind(AssertUnwindSafe(|| {
            self.clean_jobs();
            self.cache.evict();
        }));
    }

    fn clean_jobs(&self) {
        let mut jobs = lock(&self.jobs);
        let Ok(dir) = fs::read_dir(&self.settings.jobs_dir) else { return };
        let mut finished: Vec<(std::time::SystemTime, String)> = dir
            .flatten()
            .filter_map(|e| {
                let id = e.file_name().to_str().filter(|n| is_job_id(n))?.to_string();
                let last_log = fs::metadata(e.path().join("job.log")).and_then(|m| m.modified()).ok()?;
                let unfinished = jobs.get(&id).is_some_and(|j| !j.phase().is_final());
                (e.path().is_dir() && !unfinished).then_some((last_log, id))
            })
            .collect();
        finished.sort_by(|a, b| b.cmp(a));
        for (_, id) in finished.into_iter().skip(self.settings.keep_jobs.max(1)) {
            if fs::remove_dir_all(self.settings.jobs_dir.join(&id)).is_ok() {
                jobs.remove(&id);
            }
        }
    }
}

fn run(job: &Job, w: &Worker) {
    if job.cancelled() {
        return;
    }
    let outcome = pipeline(job, w);
    discard_intermediates(job);
    match outcome {
        Ok(()) => {
            write_record(job, &w.tools, "done", None);
            if job.set_phase(Phase::Done) {
                job.log("done");
            }
        }
        Err(e) => {
            job.log(&format!("error: {e}"));
            write_record(job, &w.tools, if job.cancelled() { "cancelled" } else { "failed" }, Some(&e));
            if !job.cancelled() {
                job.set_phase(Phase::Failed(e));
            }
        }
    }
}

thread_local! {
    /// The last panic on this thread: its message, and the full report with a backtrace.
    static LAST_PANIC: RefCell<Option<(String, String)>> = const { RefCell::new(None) };
}

/// Keep each panic's message and backtrace for the job it hit; the default hook still prints it.
fn install_panic_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let payload = info.payload();
            let message = payload.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| payload.downcast_ref::<String>().cloned()).unwrap_or_else(|| "no message".into());
            let report = format!("{info}\n\n{}", std::backtrace::Backtrace::force_capture());
            LAST_PANIC.with(|p| *p.borrow_mut() = Some((message, report)));
            default(info);
        }));
    });
}

/// A job whose run panicked: stop its tool, keep the report beside it, and fail it. The worker goes on.
fn crashed(job: &Job, w: &Worker) {
    let (message, report) = LAST_PANIC.with(|p| p.borrow_mut().take()).unwrap_or_else(|| ("no message".into(), "no report".into()));
    if let Some(mut child) = lock(&job.child).take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    *lock(&job.tool) = None;
    discard_intermediates(job);
    let report_path = job.dir.join("panic.log");
    let _ = fs::write(&report_path, report);
    let error = format!("sona hit an internal error ({message}); the details are in {}", report_path.display());
    job.log(&format!("error: {error}"));
    write_record(job, &w.tools, if job.cancelled() { "cancelled" } else { "failed" }, Some(&error));
    job.set_phase(Phase::Failed(error));
}

/// The separators' working folders; the stems taken from them are already in the job folder.
fn discard_intermediates(job: &Job) {
    for d in ["demucs", "uvr"] {
        let _ = fs::remove_dir_all(job.dir.join(d));
    }
}

fn step(job: &Job, phase: Phase) -> Result<(), String> {
    if job.cancelled() || !job.set_phase(phase.clone()) {
        return Err("cancelled".into());
    }
    lock(&job.steps).push((phase.label(), Instant::now()));
    job.log(phase.label());
    Ok(())
}

/// A step that leaves the phase as it is (the stems taken from the cache).
fn note_step(job: &Job, label: &'static str) {
    lock(&job.steps).push((label, Instant::now()));
}

/// The job so far: its upload, how long each step took (the last one up to now), the tools and models.
fn job_record(job: &Job, tools: &Tools) -> sona_core::JobRecord {
    let now = Instant::now();
    let starts = lock(&job.steps).clone();
    let steps = starts
        .iter()
        .enumerate()
        .map(|(i, (step, at))| sona_core::StepTime { step: (*step).into(), seconds: starts.get(i + 1).map_or(now, |n| n.1).duration_since(*at).as_secs_f64() })
        .collect();
    let upload = input_path(job)
        .and_then(|p| crate::provenance::fingerprint_file("upload", &p))
        .map(|mut f| {
            f.name = job.spec.name.clone();
            f
        })
        .unwrap_or_else(|_| crate::provenance::fingerprint_bytes("upload", &job.spec.name, &[]));
    let mut models = Vec::new();
    if job.spec.separation == Separation::Lead {
        if let Ok(f) = crate::provenance::fingerprint_file("separator/lead", &tools.separator_models.join(&tools.lead_model)) {
            models.push(f);
        }
    }
    sona_core::JobRecord { id: job.id.clone(), upload, steps, tools: tool_versions(), models, stems: lock(&job.stems).clone() }
}

static TOOL_VERSIONS: std::sync::OnceLock<BTreeMap<String, String>> = std::sync::OnceLock::new();

/// The tool versions if the start-up probe has finished. Only the stem-cache key waits for them
/// (bounded by the probe's own limits, and cancellable); the record never does.
fn tool_versions() -> BTreeMap<String, String> {
    TOOL_VERSIONS.get().cloned().unwrap_or_else(|| [("probe".to_string(), "unfinished".to_string())].into())
}

/// Read the external tools' versions once, off the job path, when the server starts.
fn probe_tool_versions(tools: Tools) {
    std::thread::spawn(move || {
        let _ = TOOL_VERSIONS.set(read_tool_versions(&tools));
    });
}

fn read_tool_versions(tools: &Tools) -> BTreeMap<String, String> {
    {
        use crate::provenance::tool_stdout;
        let limit = Duration::from_secs(20);
        let unknown = || "unknown".to_string();
        let py_versions = |python: &Path, pkgs: [&str; 2]| -> [String; 2] {
            let script = format!("import importlib.metadata as m\nfor p in {pkgs:?}:\n    try: print(m.version(p))\n    except Exception: print('unknown')");
            let out = tool_stdout(Command::new(python).args(["-c", &script]), limit).unwrap_or_default();
            let mut lines = out.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string);
            [lines.next().unwrap_or_else(unknown), lines.next().unwrap_or_else(unknown)]
        };
        let mut v = BTreeMap::new();
        let [demucs, torch] = py_versions(&tools.demucs_python, ["demucs", "torch"]);
        v.insert("demucs".into(), demucs);
        v.insert("demucs_model".into(), "htdemucs".into());
        v.insert("demucs_shift_seed".into(), "0".into());
        v.insert("demucs_torch".into(), torch);
        let [separator, ort] = py_versions(&tools.separator_bin.with_file_name("python"), ["audio-separator", "onnxruntime"]);
        v.insert("audio_separator".into(), separator);
        v.insert("audio_separator_onnxruntime".into(), ort);
        v.insert("lead_model".into(), tools.lead_model.clone());
        let ffmpeg = tool_stdout(Command::new(&tools.ffmpeg).arg("-version"), limit).unwrap_or_default();
        v.insert("ffmpeg".into(), ffmpeg.split_whitespace().nth(2).map_or_else(unknown, str::to_string));
        v
    }
}

/// Write the job's record beside its results, whatever the outcome.
fn write_record(job: &Job, tools: &Tools, outcome: &str, error: Option<&str>) {
    let record = serde_json::json!({ "outcome": outcome, "error": error, "job": job_record(job, tools) });
    if let Ok(json) = serde_json::to_vec_pretty(&record) {
        let _ = fs::write(job.provenance_path(), json);
    }
}

fn input_path(job: &Job) -> Result<PathBuf, String> {
    fs::read_dir(&job.dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| p.file_stem().is_some_and(|s| s == "input"))
        .ok_or_else(|| "the upload is missing".into())
}

/// Fault injection for `tests/serve.rs`: a job whose upload has this name panics where it would transcribe.
const PANIC_FOR_TEST: &str = "SONA_TEST_PANIC";

fn pipeline(job: &Job, w: &Worker) -> Result<(), String> {
    let input = input_path(job)?;
    *lock(&job.audio_s) = audio_seconds(&input, &w.tools);
    match job.spec.separation {
        Separation::None => {
            step(job, Phase::Decoding)?;
            to_mono_wav(job, w, &input, &job.stem_path())?;
        }
        Separation::Vocals | Separation::Lead => separate(job, w, &input)?,
    }
    step(job, Phase::Transcribing)?;
    let tools = &w.tools;
    let args = crate::TranscribeArgs {
        stem: job.stem_path().to_string_lossy().into_owned(),
        bpm: job.spec.bpm,
        midi: None,
        grid: Some(job.spec.grid),
        salience: true,
        spectral_cue: cfg!(feature = "spectral"),
        game: match job.spec.engine {
            Engine::Game | Engine::Cells => Some(tools.game_dir.as_ref().ok_or("GAME is not configured on this server")?.to_string_lossy().into_owned()),
            Engine::Pyin => None,
        },
        octave_check: true,
        vocals: (job.spec.engine == Engine::Cells).then(|| job.vocals_path().to_string_lossy().into_owned()),
        cell_model: match job.spec.engine {
            Engine::Cells => Some(tools.cell_model.as_ref().ok_or("no cell model is configured on this server")?.to_string_lossy().into_owned()),
            _ => None,
        },
    };
    if std::env::var(PANIC_FOR_TEST).is_ok_and(|name| name == job.spec.name) {
        panic!("{PANIC_FOR_TEST} asked for a panic in {}", job.spec.name);
    }
    let mut annotation = crate::transcribe(&args)?;
    if job.cancelled() {
        return Err("cancelled".into());
    }
    if let Some(p) = annotation.provenance.as_mut() {
        p.job = Some(job_record(job, tools));
    }
    let json = serde_json::to_vec(&annotation).map_err(|e| e.to_string())?;
    fs::write(job.annotation_path(), json).map_err(|e| e.to_string())?;
    job.log(&format!("{} notes", annotation.notes.len()));
    Ok(())
}

/// The files a separation leaves in the job folder, which a stem-cache entry holds. A lead job keeps the
/// vocals too, so a later cell-stage job can reuse the entry.
fn stem_files(separation: Separation) -> &'static [&'static str] {
    match separation {
        Separation::Lead => &["stem.wav", "vocals.wav"],
        _ => &["stem.wav"],
    }
}

/// Separate the upload into `stem.wav` (and `vocals.wav` for the lead), or take them from the stem cache.
fn separate(job: &Job, w: &Worker, input: &Path) -> Result<(), String> {
    let tools = &w.tools;
    let files = stem_files(job.spec.separation);
    let key = if w.cache.enabled() { stem_key(job, tools, input)? } else { None };
    if let Some(key) = &key {
        if let Some(by) = w.cache.restore(key, files, &job.dir) {
            note_step(job, "reused-stems");
            job.log(&format!("separation reused from job {by} (stem cache {})", &key.hex[..12]));
            *lock(&job.stems) = Some(sona_core::StemSource { key: key.hex.clone(), reused: true, separated_by: by });
            return Ok(());
        }
    }
    step(job, Phase::SeparatingVocals)?;
    let out = job.dir.join("demucs");
    let mut cmd = Command::new(&tools.demucs_python);
    ffmpeg_on_path(&mut cmd, &tools.ffmpeg);
    cmd.args(["-c", DEMUCS_SEEDED, "-n", "htdemucs", "--two-stems", "vocals", "-o"]).arg(&out).arg(input);
    run_child(job, w, cmd, "demucs")?;
    let vocals = find_file(&out, |n| n == "vocals.wav").ok_or("demucs wrote no vocals.wav")?;
    if job.spec.separation == Separation::Vocals {
        to_mono_wav(job, w, &vocals, &job.stem_path())?;
    } else {
        to_mono_wav(job, w, &vocals, &job.vocals_path())?;
        step(job, Phase::SeparatingLead)?;
        let out = job.dir.join("uvr");
        let mut cmd = Command::new(&tools.separator_bin);
        ffmpeg_on_path(&mut cmd, &tools.ffmpeg);
        cmd.arg(&vocals)
            .args(["--model_filename", &tools.lead_model, "--model_file_dir"])
            .arg(&tools.separator_models)
            .args(["--output_dir"])
            .arg(&out)
            .args(["--output_format", "WAV", "--single_stem", "Vocals"]);
        run_child(job, w, cmd, "audio-separator")?;
        let lead = find_file(&out, |n| n.to_lowercase().contains("vocals") && n.ends_with(".wav")).ok_or("the lead separator wrote no Vocals WAV")?;
        to_mono_wav(job, w, &lead, &job.stem_path())?;
    }
    if let Some(key) = key {
        if job.cancelled() {
            return Err("cancelled".into());
        }
        match w.cache.store(&key, files, &job.dir, &job.id) {
            Ok(()) => job.log(&format!("stems cached ({})", &key.hex[..12])),
            Err(e) => job.log(&format!("could not cache the stems: {e}")),
        }
        *lock(&job.stems) = Some(sona_core::StemSource { key: key.hex, reused: false, separated_by: job.id.clone() });
    }
    Ok(())
}

/// The separators call `ffmpeg` from `PATH`; put the folder of the one sona was given (`SONA_FFMPEG`) first.
fn ffmpeg_on_path(cmd: &mut Command, ffmpeg: &Path) {
    let Some(dir) = ffmpeg.parent().filter(|d| !d.as_os_str().is_empty()) else { return };
    let path = std::env::var_os("PATH").unwrap_or_default();
    if let Ok(joined) = std::env::join_paths(std::iter::once(dir.to_path_buf()).chain(std::env::split_paths(&path))) {
        cmd.env("PATH", joined);
    }
}

/// The stem-cache key: the upload's hash, the separation, and every separator version, model and
/// setting that shapes the stems. `None` (no caching) if the tool versions are still unknown.
fn stem_key(job: &Job, tools: &Tools, input: &Path) -> Result<Option<StemKey>, String> {
    let Some(versions) = wait_for_tool_versions(job)? else {
        job.log("stem cache skipped: the separators' versions couldn't be read in time");
        return Ok(None);
    };
    let upload = crate::provenance::fingerprint_file("upload", input)?;
    let mut fields = BTreeMap::new();
    let mut set = |k: &str, v: String| {
        fields.insert(k.to_string(), v);
    };
    let version = |name: &str| versions.get(name).cloned().unwrap_or_else(|| "unknown".into());
    set("format", "1".into());
    set("upload_sha256", upload.sha256);
    set("separation", if job.spec.separation == Separation::Lead { "lead" } else { "vocals" }.into());
    set("demucs_python", tools.demucs_python.display().to_string());
    for name in ["demucs", "demucs_model", "demucs_shift_seed", "demucs_torch", "ffmpeg"] {
        set(name, version(name));
    }
    if job.spec.separation == Separation::Lead {
        for name in ["audio_separator", "audio_separator_onnxruntime", "lead_model"] {
            set(name, version(name));
        }
        let model = crate::provenance::fingerprint_file("separator/lead", &tools.separator_models.join(&tools.lead_model));
        set("lead_model_sha256", model.map_or_else(|_| "missing".into(), |f| f.sha256));
        set("separator_bin", tools.separator_bin.display().to_string());
    }
    Ok(Some(StemKey::new(fields)))
}

/// The start-up probe's tool versions, waiting for it if it is still running. The probe gives each tool
/// 20 s, so the wait is bounded; a cancel ends it.
fn wait_for_tool_versions(job: &Job) -> Result<Option<BTreeMap<String, String>>, String> {
    let t0 = Instant::now();
    loop {
        if let Some(v) = TOOL_VERSIONS.get() {
            return Ok(Some(v.clone()));
        }
        if job.cancelled() {
            return Err("cancelled".into());
        }
        if t0.elapsed() > Duration::from_secs(90) {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The upload's length in seconds: exact for a WAV, ffmpeg's reading otherwise, and failing both, its
/// size at 128 kbit/s.
fn audio_seconds(input: &Path, tools: &Tools) -> f64 {
    if let Ok(r) = hound::WavReader::open(input) {
        return f64::from(r.duration()) / f64::from(r.spec().sample_rate.max(1));
    }
    ffmpeg_duration(tools, input).unwrap_or_else(|| fs::metadata(input).map_or(0.0, |m| m.len() as f64 / 16_000.0))
}

fn ffmpeg_duration(tools: &Tools, input: &Path) -> Option<f64> {
    use std::io::Read;
    let mut child = Command::new(&tools.ffmpeg).args(["-nostdin", "-hide_banner", "-i"]).arg(input).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().ok()?;
    let t0 = Instant::now();
    while child.try_wait().ok()?.is_none() {
        if t0.elapsed() > Duration::from_secs(20) {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut text = String::new();
    child.stderr.take()?.read_to_string(&mut text).ok()?;
    let at = text.find("Duration: ")? + "Duration: ".len();
    let hms: Vec<f64> = text[at..].split(',').next()?.trim().split(':').map(|p| p.parse().ok()).collect::<Option<_>>()?;
    match hms.as_slice() {
        [h, m, s] => Some(h * 3600.0 + m * 60.0 + s),
        _ => None,
    }
}

/// demucs with its random time shift seeded. Its default (one shift of up to 0.5 s from Python's unseeded
/// `random`) is what the corpus stems used, so the method stays the same and only the draw is fixed
/// (ADR-0024).
const DEMUCS_SEEDED: &str = "import random, sys; random.seed(0); from demucs.separate import main; main(sys.argv[1:])";

/// The last `n` lines a tool printed; carriage returns end lines too, so a progress bar reads as lines.
fn log_tail(path: &Path, n: usize) -> String {
    let text = fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<&str> = text.split(['\r', '\n']).map(str::trim).filter(|l| !l.is_empty()).collect();
    lines[lines.len().saturating_sub(n)..].join(" | ")
}

fn run_child(job: &Job, w: &Worker, mut cmd: Command, label: &'static str) -> Result<(), String> {
    let log_path = job.dir.join(format!("{label}.log"));
    let log = fs::File::create(&log_path).map_err(|e| e.to_string())?;
    let child = cmd
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|e| e.to_string())?)
        .stderr(log)
        .spawn()
        .map_err(|e| format!("could not start {label} ({}): {e}", cmd.get_program().to_string_lossy()))?;
    *lock(&job.child) = Some(child);
    *lock(&job.tool) = Some(label);
    let audio_s = *lock(&job.audio_s);
    let limit = w.settings.tool_timeout.for_audio(audio_s);
    let started = Instant::now();
    let mut overran = false;
    let status = loop {
        let overdue = started.elapsed() > limit;
        overran |= overdue;
        if job.cancelled() || overdue {
            if let Some(c) = lock(&job.child).as_mut() {
                let _ = c.kill();
            }
        }
        let polled = lock(&job.child).as_mut().map(|c| c.try_wait());
        match polled {
            Some(Ok(Some(status))) => break Ok(status),
            Some(Ok(None)) => std::thread::sleep(Duration::from_millis(100)),
            Some(Err(e)) => break Err(e.to_string()),
            None => break Err("cancelled".to_string()),
        }
    };
    *lock(&job.child) = None;
    *lock(&job.tool) = None;
    let status = status?;
    if job.cancelled() {
        return Err("cancelled".into());
    }
    if overran {
        return Err(format!(
            "{label} was stopped after {:.0} s, its time limit for {audio_s:.0} s of audio. It last printed: {}. If the machine is only slow, start sona serve with a larger --tool-timeout or --tool-timeout-ratio",
            limit.as_secs_f64(),
            log_tail(&log_path, 3)
        ));
    }
    if !status.success() {
        return Err(format!("{label} failed ({status}): {}", log_tail(&log_path, 8)));
    }
    Ok(())
}

fn find_file(root: &Path, want: impl Fn(&str) -> bool + Copy) -> Option<PathBuf> {
    for entry in fs::read_dir(root).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file(&path, want) {
                return Some(found);
            }
        } else if path.file_name().and_then(|n| n.to_str()).is_some_and(want) {
            return Some(path);
        }
    }
    None
}

/// Every stem is 44.1 kHz: GAME needs it, and pYIN's frames are sized in samples.
pub const STEM_RATE: u32 = 44_100;

/// The sample rate of an upload that is a WAV; anything else needs ffmpeg to be read.
pub fn wav_rate(audio: &[u8]) -> Option<u32> {
    hound::WavReader::new(std::io::Cursor::new(audio)).ok().map(|r| r.spec().sample_rate)
}

/// A mono stem at [`STEM_RATE`]: a WAV at that rate is downmixed in-process; anything else goes
/// through ffmpeg.
fn to_mono_wav(job: &Job, w: &Worker, src: &Path, dst: &Path) -> Result<(), String> {
    let wav_rate = match hound::WavReader::open(src) {
        Ok(reader) if reader.spec().sample_rate == STEM_RATE => return downmix_wav(reader, dst),
        Ok(reader) => Some(reader.spec().sample_rate),
        Err(_) => None,
    };
    let mut cmd = Command::new(&w.tools.ffmpeg);
    cmd.args(["-nostdin", "-loglevel", "error", "-y", "-i"]).arg(src).args(["-ac", "1", "-ar", &STEM_RATE.to_string(), "-c:a", "pcm_s16le"]).arg(dst);
    run_child(job, w, cmd, "ffmpeg").map_err(|e| match wav_rate {
        Some(rate) if e != "cancelled" => format!("resampling the {rate} Hz WAV to {STEM_RATE} Hz: {e}"),
        _ => e,
    })
}

fn downmix_wav<R: std::io::Read>(reader: hound::WavReader<R>, dst: &Path) -> Result<(), String> {
    let (mono, sample_rate) = read_mono(reader)?;
    let out_spec = hound::WavSpec { channels: 1, sample_rate, bits_per_sample: 32, sample_format: hound::SampleFormat::Float };
    let mut w = hound::WavWriter::create(dst, out_spec).map_err(|e| e.to_string())?;
    for v in mono {
        w.write_sample(v).map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())
}

fn read_mono<R: std::io::Read>(mut reader: hound::WavReader<R>) -> Result<(Vec<f32>, u32), String> {
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>().map_err(|e| e.to_string())?,
        hound::SampleFormat::Int => {
            let full = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader.samples::<i32>().map(|s| s.map(|v| v as f32 / full)).collect::<Result<_, _>>().map_err(|e| e.to_string())?
        }
    };
    let ch = spec.channels.max(1) as usize;
    Ok((samples.chunks(ch).map(|f| f.iter().sum::<f32>() / f.len() as f32).collect(), spec.sample_rate))
}

#[cfg(feature = "spectral")]
pub fn song_tempo(tools: &Tools, scratch: &Path, ext: &str, audio: &[u8]) -> Result<Option<f64>, String> {
    let (mono, sr) = match hound::WavReader::new(std::io::Cursor::new(audio)) {
        Ok(reader) => read_mono(reader)?,
        Err(_) => {
            fs::create_dir_all(scratch).map_err(|e| e.to_string())?;
            let tag = format!("tempo-{}-{}", std::process::id(), NEXT_TEMPO.fetch_add(1, Ordering::Relaxed));
            let src = scratch.join(format!("{tag}.{ext}"));
            let wav = scratch.join(format!("{tag}.wav"));
            fs::write(&src, audio).map_err(|e| e.to_string())?;
            let out = Command::new(&tools.ffmpeg)
                .args(["-nostdin", "-loglevel", "error", "-y", "-i"])
                .arg(&src)
                .args(["-ac", "1", "-ar", "22050", "-c:a", "pcm_s16le"])
                .arg(&wav)
                .output();
            let decoded = match out {
                Ok(o) if o.status.success() => hound::WavReader::open(&wav).map_err(|e| e.to_string()).and_then(read_mono),
                Ok(o) => Err(format!("ffmpeg could not read the file: {}", ffmpeg_reason(&String::from_utf8_lossy(&o.stderr), &src))),
                Err(e) => Err(format!("running ffmpeg: {e}")),
            };
            let _ = fs::remove_file(&src);
            let _ = fs::remove_file(&wav);
            decoded?
        }
    };
    Ok(sona_core::tempo::estimate_bpm(&mono, sr))
}

/// ffmpeg's last line (its summary), naming the upload "the file" instead of the server's scratch path.
#[cfg(feature = "spectral")]
fn ffmpeg_reason(stderr: &str, src: &Path) -> String {
    let path = src.display().to_string();
    let last = stderr.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or("no reason given");
    last.replace(&format!("{path}: "), "").replace(&path, "the file")
}

#[cfg(feature = "spectral")]
static NEXT_TEMPO: AtomicU64 = AtomicU64::new(0);
