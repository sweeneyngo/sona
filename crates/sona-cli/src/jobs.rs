use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct Tools {
    pub demucs_python: PathBuf,
    pub separator_bin: PathBuf,
    pub separator_models: PathBuf,
    pub lead_model: String,
    pub ffmpeg: PathBuf,
    pub game_dir: Option<PathBuf>,
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
    /// When each step began, in order (ADR-0024).
    steps: Mutex<Vec<(&'static str, Instant)>>,
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

    fn log(&self, line: &str) {
        use std::io::Write;
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(self.dir.join("job.log")) {
            let _ = writeln!(f, "[{:6.1}s] {line}", self.created.elapsed().as_secs_f64());
        }
    }
}

pub struct Queue {
    jobs: Mutex<HashMap<String, Arc<Job>>>,
    tx: Mutex<Sender<Arc<Job>>>,
    root: PathBuf,
    counter: AtomicU64,
}

impl Queue {
    pub fn start(root: PathBuf, tools: Tools) -> std::io::Result<Arc<Queue>> {
        fs::create_dir_all(&root)?;
        probe_tool_versions(tools.clone());
        let (tx, rx) = channel::<Arc<Job>>();
        let q = Arc::new(Queue { jobs: Mutex::new(HashMap::new()), tx: Mutex::new(tx), root, counter: AtomicU64::new(0) });
        std::thread::spawn(move || {
            for job in rx {
                run(&job, &tools);
            }
        });
        Ok(q)
    }

    pub fn submit(&self, spec: JobSpec, input_ext: &str, audio: &[u8]) -> std::io::Result<Arc<Job>> {
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
            steps: Mutex::new(vec![(Phase::Queued.label(), Instant::now())]),
        });
        job.log(&format!("received {} bytes", audio.len()));
        lock(&self.jobs).insert(id, job.clone());
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

fn run(job: &Job, tools: &Tools) {
    if job.cancelled() {
        return;
    }
    match pipeline(job, tools) {
        Ok(()) => {
            write_record(job, tools, "done", None);
            if job.set_phase(Phase::Done) {
                job.log("done");
            }
        }
        Err(e) => {
            job.log(&format!("error: {e}"));
            write_record(job, tools, if job.cancelled() { "cancelled" } else { "failed" }, Some(&e));
            if !job.cancelled() {
                job.set_phase(Phase::Failed(e));
            }
        }
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
    sona_core::JobRecord { id: job.id.clone(), upload, steps, tools: tool_versions(), models }
}

static TOOL_VERSIONS: std::sync::OnceLock<std::collections::BTreeMap<String, String>> = std::sync::OnceLock::new();

/// The tool versions if the start-up probe has finished; a job never waits for it (a hung tool would
/// hold the queue).
fn tool_versions() -> std::collections::BTreeMap<String, String> {
    TOOL_VERSIONS.get().cloned().unwrap_or_else(|| [("probe".to_string(), "unfinished".to_string())].into())
}

/// Read the external tools' versions once, off the job path, when the server starts.
fn probe_tool_versions(tools: Tools) {
    std::thread::spawn(move || {
        let _ = TOOL_VERSIONS.set(read_tool_versions(&tools));
    });
}

fn read_tool_versions(tools: &Tools) -> std::collections::BTreeMap<String, String> {
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
        let mut v = std::collections::BTreeMap::new();
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

fn pipeline(job: &Job, tools: &Tools) -> Result<(), String> {
    let input = input_path(job)?;
    let separated = match job.spec.separation {
        Separation::None => {
            step(job, Phase::Decoding)?;
            to_mono_wav(job, tools, &input, &job.stem_path())?;
            None
        }
        Separation::Vocals | Separation::Lead => {
            step(job, Phase::SeparatingVocals)?;
            let out = job.dir.join("demucs");
            let mut cmd = Command::new(&tools.demucs_python);
            cmd.args(["-c", DEMUCS_SEEDED, "-n", "htdemucs", "--two-stems", "vocals", "-o"]).arg(&out).arg(&input);
            run_child(job, cmd, "demucs")?;
            let vocals = find_file(&out, |n| n == "vocals.wav").ok_or("demucs wrote no vocals.wav")?;
            if job.spec.separation == Separation::Vocals {
                Some(vocals)
            } else {
                step(job, Phase::SeparatingLead)?;
                let out = job.dir.join("uvr");
                let mut cmd = Command::new(&tools.separator_bin);
                cmd.arg(&vocals)
                    .args(["--model_filename", &tools.lead_model, "--model_file_dir"])
                    .arg(&tools.separator_models)
                    .args(["--output_dir"])
                    .arg(&out)
                    .args(["--output_format", "WAV", "--single_stem", "Vocals"]);
                run_child(job, cmd, "audio-separator")?;
                Some(find_file(&out, |n| n.to_lowercase().contains("vocals") && n.ends_with(".wav")).ok_or("the lead separator wrote no Vocals WAV")?)
            }
        }
    };
    if let Some(stem) = separated {
        to_mono_wav(job, tools, &stem, &job.stem_path())?;
    }
    step(job, Phase::Transcribing)?;
    let args = crate::TranscribeArgs {
        stem: job.stem_path().to_string_lossy().into_owned(),
        bpm: job.spec.bpm,
        midi: None,
        grid: Some(job.spec.grid),
        salience: true,
        spectral_cue: cfg!(feature = "spectral"),
        game: match job.spec.engine {
            Engine::Game => Some(tools.game_dir.as_ref().ok_or("GAME is not configured on this server")?.to_string_lossy().into_owned()),
            Engine::Pyin => None,
        },
        octave_check: true,
    };
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

/// demucs with its random time shift seeded. Its default (one shift of up to 0.5 s from Python's unseeded
/// `random`) is what the corpus stems used, so the method stays the same and only the draw is fixed
/// (ADR-0024).
const DEMUCS_SEEDED: &str = "import random, sys; random.seed(0); from demucs.separate import main; main(sys.argv[1:])";

fn run_child(job: &Job, mut cmd: Command, label: &str) -> Result<(), String> {
    let log_path = job.dir.join(format!("{label}.log"));
    let log = fs::File::create(&log_path).map_err(|e| e.to_string())?;
    let child = cmd
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|e| e.to_string())?)
        .stderr(log)
        .spawn()
        .map_err(|e| format!("could not start {label} ({}): {e}", cmd.get_program().to_string_lossy()))?;
    *lock(&job.child) = Some(child);
    let status = loop {
        if job.cancelled() {
            if let Some(c) = lock(&job.child).as_mut() {
                let _ = c.kill();
            }
        }
        let polled = lock(&job.child).as_mut().map(|c| c.try_wait());
        match polled {
            Some(Ok(Some(status))) => break status,
            Some(Ok(None)) => std::thread::sleep(Duration::from_millis(100)),
            Some(Err(e)) => return Err(e.to_string()),
            None => return Err("cancelled".into()),
        }
    };
    *lock(&job.child) = None;
    if job.cancelled() {
        return Err("cancelled".into());
    }
    if !status.success() {
        let tail: Vec<String> = fs::read_to_string(&log_path).unwrap_or_default().lines().rev().take(8).map(str::to_string).collect();
        return Err(format!("{label} failed ({status}): {}", tail.into_iter().rev().collect::<Vec<_>>().join(" | ")));
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

fn to_mono_wav(job: &Job, tools: &Tools, src: &Path, dst: &Path) -> Result<(), String> {
    if let Ok(reader) = hound::WavReader::open(src) {
        return downmix_wav(reader, dst);
    }
    let mut cmd = Command::new(&tools.ffmpeg);
    cmd.args(["-nostdin", "-loglevel", "error", "-y", "-i"]).arg(src).args(["-ac", "1", "-ar", "44100", "-c:a", "pcm_s16le"]).arg(dst);
    run_child(job, cmd, "ffmpeg")
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
                Ok(o) => Err(format!("ffmpeg could not read the file: {}", String::from_utf8_lossy(&o.stderr).trim())),
                Err(e) => Err(format!("running ffmpeg: {e}")),
            };
            let _ = fs::remove_file(&src);
            let _ = fs::remove_file(&wav);
            decoded?
        }
    };
    Ok(sona_core::tempo::estimate_bpm(&mono, sr))
}

#[cfg(feature = "spectral")]
static NEXT_TEMPO: AtomicU64 = AtomicU64::new(0);
