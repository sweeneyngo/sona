use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use crate::http::{read_request, write_response, HttpError, Limits, Request, Response};
use crate::jobs::{wav_rate, Engine, Job, JobSpec, Phase, Queue, Separation, Settings, ToolTimeout, Tools, STEM_RATE};

const LIMITS: Limits = Limits { max_header_bytes: 16 * 1024, max_body_bytes: 500 * 1_000_000 };
const DEFAULT_PORT: u16 = 8977;
const MAX_BPM: f64 = 400.0;

pub struct ServeArgs {
    port: u16,
    web_dir: Option<PathBuf>,
    settings: Settings,
    tools: Tools,
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

/// `$XDG_CACHE_HOME/sona`, else `~/.cache/sona`: on disk, where `/tmp` may be RAM (ADR-0030).
fn default_cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).filter(|d| d.is_absolute()).unwrap_or_else(|| home().join(".cache")).join("sona")
}

/// Where demucs 4.0.1's torch keeps the htdemucs weights it downloads on first use: `$TORCH_HOME/hub/checkpoints`,
/// `TORCH_HOME` defaulting to `$XDG_CACHE_HOME/torch`, else `~/.cache/torch`.
fn demucs_weights() -> PathBuf {
    let torch = std::env::var_os("TORCH_HOME").map(PathBuf::from).unwrap_or_else(|| std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".cache")).join("torch"));
    torch.join("hub/checkpoints/955717e8-8726e21a.th")
}

pub fn parse_serve_args(args: &[String]) -> Result<ServeArgs, String> {
    let share = home().join(".local/share/sona");
    let mut port = DEFAULT_PORT;
    let mut web_dir: Option<PathBuf> = None;
    let mut cache_dir = default_cache_dir();
    let mut jobs_dir: Option<PathBuf> = None;
    let (mut keep_jobs, mut keep_stems) = (20usize, 10usize);
    let mut tool_timeout = ToolTimeout { min_s: 600.0, per_audio_s: 20.0 };
    let mut demucs_venv = std::env::var_os("SONA_DEMUCS_VENV").map(PathBuf::from).unwrap_or_else(|| share.join("demucs-venv"));
    let mut separator_venv = std::env::var_os("SONA_LEAD_VENV").map(PathBuf::from).unwrap_or_else(|| share.join("separator-venv"));
    let mut game_dir: Option<PathBuf> = None;
    let mut cell_model: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        let value = |i: usize| args.get(i + 1).cloned().ok_or_else(|| format!("{} expects a value", args[i]));
        match args[i].as_str() {
            "--port" => port = value(i)?.parse().map_err(|_| "--port must be a number (0 picks a free port)")?,
            "--web-dir" => web_dir = Some(PathBuf::from(value(i)?)),
            "--cache-dir" => cache_dir = PathBuf::from(value(i)?),
            "--jobs-dir" => jobs_dir = Some(PathBuf::from(value(i)?)),
            "--keep-jobs" => keep_jobs = value(i)?.parse().ok().filter(|n| *n >= 1).ok_or("--keep-jobs must be a whole number, at least 1")?,
            "--keep-stems" => keep_stems = value(i)?.parse().map_err(|_| "--keep-stems must be a whole number (0 turns the stem cache off)")?,
            "--tool-timeout" => tool_timeout.min_s = value(i)?.parse().ok().filter(|s: &f64| s.is_finite() && *s > 0.0).ok_or("--tool-timeout must be a positive number of seconds")?,
            "--tool-timeout-ratio" => {
                tool_timeout.per_audio_s = value(i)?.parse().ok().filter(|r: &f64| r.is_finite() && *r >= 0.0).ok_or("--tool-timeout-ratio must be a number of seconds per second of audio")?
            }
            "--demucs-venv" => demucs_venv = PathBuf::from(value(i)?),
            "--separator-venv" => separator_venv = PathBuf::from(value(i)?),
            "--game" => game_dir = Some(PathBuf::from(value(i)?)),
            "--cell-model" => cell_model = Some(PathBuf::from(value(i)?)),
            other => return Err(format!("unknown serve option '{other}'")),
        }
        i += 2;
    }
    let web_dir = web_dir.or_else(|| {
        let built = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/dist");
        built.join("index.html").is_file().then_some(built)
    });
    if game_dir.is_some() && !cfg!(feature = "game") {
        return Err("--game needs sona built with `--features game`".into());
    }
    if cell_model.is_some() && game_dir.is_none() {
        return Err("--cell-model needs --game DIR too".into());
    }
    let tools = Tools {
        demucs_python: demucs_venv.join("bin/python"),
        separator_bin: separator_venv.join("bin/audio-separator"),
        separator_models: separator_venv.join("models"),
        lead_model: std::env::var("SONA_LEAD_MODEL").unwrap_or_else(|_| "UVR_MDXNET_KARA_2.onnx".into()),
        ffmpeg: std::env::var_os("SONA_FFMPEG").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("ffmpeg")),
        demucs_weights: demucs_weights(),
        game_dir,
        cell_model,
    };
    let settings = Settings { jobs_dir: jobs_dir.unwrap_or_else(|| cache_dir.join("jobs")), stems_dir: cache_dir.join("stems"), keep_jobs, keep_stems, tool_timeout };
    Ok(ServeArgs { port, web_dir, settings, tools })
}

struct Server {
    port: u16,
    web_dir: Option<PathBuf>,
    tools: Tools,
    /// Whether `ffmpeg -version` ran at start-up.
    ffmpeg_runs: bool,
    queue: Arc<Queue>,
}

const FFMPEG_FIX: &str = "install ffmpeg and restart sona serve";

/// Something separation needs that isn't there yet. A missing piece turns off the separations it
/// `blocks`; a first-use one only means a download when a job first needs it.
struct SetupNote {
    piece: &'static str,
    missing: bool,
    blocks: &'static [&'static str],
    message: String,
}

pub fn run_serve(args: &[String]) -> Result<(), String> {
    let a = parse_serve_args(args)?;
    let listener = TcpListener::bind(("127.0.0.1", a.port)).map_err(|e| format!("binding 127.0.0.1:{}: {e}", a.port))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let jobs_dir = a.settings.jobs_dir.clone();
    let ffmpeg_runs = crate::provenance::tool_stdout(Command::new(&a.tools.ffmpeg).arg("-version"), Duration::from_secs(5)).is_some();
    let queue = Queue::start(a.settings, a.tools.clone()).map_err(|e| format!("jobs dir {}: {e}", jobs_dir.display()))?;
    let server = Arc::new(Server { port, web_dir: a.web_dir, tools: a.tools, ffmpeg_runs, queue });
    println!("sona: serving http://127.0.0.1:{port}");
    if server.web_dir.is_none() {
        println!("sona: no web build found (run `npm run build` in web/, or pass --web-dir); the API still works");
    }
    use std::io::Write;
    let _ = std::io::stdout().flush();
    for note in server.setup() {
        let _ = writeln!(std::io::stderr(), "sona: {}", note.message);
    }
    for stream in listener.incoming().flatten() {
        let server = server.clone();
        std::thread::spawn(move || server.handle(stream));
    }
    Ok(())
}

impl Server {
    fn handle(&self, stream: TcpStream) {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(120)));
        let response = match read_request(&stream, &LIMITS) {
            Ok(req) => self.route(&req).unwrap_or_else(|e| Response::error(&e)),
            Err(e) => Response::error(&e),
        };
        let _ = write_response(&stream, &response);
    }

    /// What separation still needs: ffmpeg as probed at start-up, and the venvs and model files as they
    /// are now, so running scripts/setup-separators.sh or a first download shows without a restart.
    fn setup(&self) -> Vec<SetupNote> {
        let t = &self.tools;
        let mut notes = Vec::new();
        if !self.ffmpeg_runs {
            notes.push(SetupNote {
                piece: "ffmpeg",
                missing: true,
                blocks: &["vocals", "lead"],
                message: format!("ffmpeg doesn't run ({}): {FFMPEG_FIX}. Until then, separation is off and only 44.1 kHz WAV files can be read.", t.ffmpeg.display()),
            });
        }
        if !t.demucs_python.is_file() {
            notes.push(SetupNote {
                piece: "demucs",
                missing: true,
                blocks: &["vocals", "lead"],
                message: format!("Vocal separation isn't installed (no {}): run scripts/setup-separators.sh, or start sona serve with --demucs-venv DIR.", t.demucs_python.display()),
            });
        } else if !t.demucs_weights.is_file() {
            notes.push(SetupNote {
                piece: "demucs-weights",
                missing: false,
                blocks: &[],
                message: "The demucs weights (84 MB) download during the first separation, which needs the internet; scripts/setup-separators.sh fetches them now.".into(),
            });
        }
        if !t.separator_bin.is_file() {
            notes.push(SetupNote {
                piece: "separator",
                missing: true,
                blocks: &["lead"],
                message: format!("Lead isolation isn't installed (no {}): run scripts/setup-separators.sh lead, or start sona serve with --separator-venv DIR.", t.separator_bin.display()),
            });
        } else if !t.separator_models.join(&t.lead_model).is_file() {
            notes.push(SetupNote {
                piece: "lead-model",
                missing: false,
                blocks: &[],
                message: format!("The lead model {} downloads during the first lead isolation, which needs the internet; scripts/setup-separators.sh lead fetches it now.", t.lead_model),
            });
        }
        notes.sort_by_key(|n| !n.missing);
        notes
    }

    fn capabilities(&self) -> serde_json::Value {
        let notes = self.setup();
        let open = |sep: &str| !notes.iter().any(|n| n.blocks.contains(&sep));
        let (vocals, lead) = (open("vocals"), open("lead"));
        let game = cfg!(feature = "game") && self.tools.game_dir.is_some() && std::env::var_os("ORT_DYLIB_PATH").is_some();
        let cells = game && lead && self.tools.cell_model.as_ref().is_some_and(|d| d.join("cellstage.onnx").is_file() && d.join("cellstage.json").is_file());
        serde_json::json!({
            "separation": { "none": true, "vocals": vocals, "lead": lead },
            "engines": { "pyin": true, "game": game, "cells": cells },
            "tempo": cfg!(feature = "spectral"),
            "setup": notes.iter().map(|n| serde_json::json!({
                "piece": n.piece,
                "status": if n.missing { "missing" } else { "first-use" },
                "blocks": n.blocks,
                "message": n.message,
            })).collect::<Vec<_>>(),
        })
    }

    fn route(&self, req: &Request) -> Result<Response, HttpError> {
        // A DNS-rebinding page reaches 127.0.0.1 under its own hostname, and the browser sends that name as `Host`.
        let host_ok = req.header("Host").is_some_and(|h| h == format!("127.0.0.1:{}", self.port) || h == format!("localhost:{}", self.port));
        if !host_ok {
            return Err(HttpError { status: 403, message: "this server only answers its own address".into() });
        }
        // A page on another site can still send a "simple" cross-origin POST; the browser names that site in `Origin`.
        let own_origin = |o: &str| o == format!("http://127.0.0.1:{}", self.port) || o == format!("http://localhost:{}", self.port);
        if req.path.starts_with("/api/") && req.header("Origin").is_some_and(|o| !own_origin(o)) {
            return Err(HttpError { status: 403, message: "this server only answers its own pages".into() });
        }
        let parts: Vec<&str> = req.path.trim_start_matches('/').split('/').collect();
        match (req.method.as_str(), parts.as_slice()) {
            ("GET", ["api", "capabilities"]) => Ok(Response::json(200, &self.capabilities())),
            ("POST", ["api", "jobs"]) => self.create_job(req),
            ("POST", ["api", "tempo"]) => self.tempo(req),
            ("GET", ["api", "jobs", id]) => {
                let job = self.job(id)?;
                Ok(Response::json(200, &job_status(&job)))
            }
            ("DELETE", ["api", "jobs", id]) => {
                let job = self.job(id)?;
                job.cancel();
                Ok(Response::json(202, &job_status(&job)))
            }
            ("GET", ["api", "jobs", id, "annotation"]) => self.job_file(id, |j| j.annotation_path(), "application/json"),
            ("GET", ["api", "jobs", id, "stem"]) => self.job_file(id, |j| j.stem_path(), "audio/wav"),
            ("GET", ["api", "jobs", id, "provenance"]) => self.job_record(id),
            (_, ["api", ..]) => Err(HttpError { status: 404, message: "no such endpoint".into() }),
            ("GET", _) => self.static_file(&req.path),
            _ => Err(HttpError { status: 405, message: "method not allowed".into() }),
        }
    }

    fn job(&self, id: &str) -> Result<Arc<Job>, HttpError> {
        self.queue.get(id).ok_or(HttpError { status: 404, message: "no such job".into() })
    }

    fn job_file(&self, id: &str, path: impl Fn(&Job) -> PathBuf, content_type: &'static str) -> Result<Response, HttpError> {
        let job = self.job(id)?;
        if job.phase() != Phase::Done {
            return Err(HttpError { status: 409, message: format!("the job is {}", job.phase().label()) });
        }
        let body = std::fs::read(path(&job)).map_err(|e| HttpError { status: 500, message: e.to_string() })?;
        Ok(Response { status: 200, content_type, body })
    }

    /// The job's record (ADR-0024), for a failed or cancelled job too, once it has finished.
    fn job_record(&self, id: &str) -> Result<Response, HttpError> {
        let job = self.job(id)?;
        if !job.phase().is_final() {
            return Err(HttpError { status: 409, message: format!("the job is {}", job.phase().label()) });
        }
        let body = std::fs::read(job.provenance_path()).map_err(|_| HttpError { status: 404, message: "this job left no record".into() })?;
        Ok(Response { status: 200, content_type: "application/json", body })
    }

    fn create_job(&self, req: &Request) -> Result<Response, HttpError> {
        let spec = job_spec(req)?;
        let caps = self.capabilities();
        let sep_key = match spec.separation {
            Separation::None => "none",
            Separation::Vocals => "vocals",
            Separation::Lead => "lead",
        };
        if caps["separation"][sep_key] != true {
            let fixes: Vec<String> = self.setup().into_iter().filter(|n| n.blocks.contains(&sep_key)).map(|n| n.message).collect();
            return Err(HttpError { status: 409, message: format!("separation '{sep_key}' isn't set up on this server. {} See `sona serve --help`.", fixes.join(" ")) });
        }
        if spec.engine == Engine::Game && caps["engines"]["game"] != true {
            return Err(HttpError { status: 409, message: "GAME isn't set up on this server (start it with --game DIR and ORT_DYLIB_PATH)".into() });
        }
        if spec.engine == Engine::Cells && (caps["engines"]["cells"] != true || spec.separation != Separation::Lead) {
            return Err(HttpError { status: 409, message: "the learned cell stage needs --cell-model DIR on the server and lead separation".into() });
        }
        if req.body.is_empty() {
            return Err(HttpError { status: 400, message: "send the audio file as the request body".into() });
        }
        if !self.ffmpeg_runs && wav_rate(&req.body) != Some(STEM_RATE) {
            return Err(HttpError { status: 409, message: format!("reading this file needs ffmpeg, which doesn't run on this server: {FFMPEG_FIX}, or drop a 44.1 kHz WAV") });
        }
        let ext = upload_ext(&spec.name);
        let job = self.queue.submit(spec, &ext, &req.body).map_err(|e| HttpError { status: 500, message: e.to_string() })?;
        Ok(Response::json(201, &job_status(&job)))
    }

    #[cfg(feature = "spectral")]
    fn tempo(&self, req: &Request) -> Result<Response, HttpError> {
        if req.body.is_empty() {
            return Err(HttpError { status: 400, message: "send the audio file as the request body".into() });
        }
        if !self.ffmpeg_runs && wav_rate(&req.body).is_none() {
            return Err(HttpError { status: 422, message: format!("reading this file needs ffmpeg, which doesn't run on this server: {FFMPEG_FIX}, or drop a WAV") });
        }
        let ext = upload_ext(req.param("name").unwrap_or("upload.wav"));
        let bpm = crate::jobs::song_tempo(&self.tools, &self.queue.root().join("scratch"), &ext, &req.body).map_err(|message| HttpError { status: 422, message })?;
        Ok(Response::json(200, &serde_json::json!({ "bpm": bpm.map(|b| (b * 10.0).round() / 10.0) })))
    }

    #[cfg(not(feature = "spectral"))]
    fn tempo(&self, _: &Request) -> Result<Response, HttpError> {
        Err(HttpError { status: 409, message: "tempo estimation needs a build with the spectral feature".into() })
    }

    fn static_file(&self, path: &str) -> Result<Response, HttpError> {
        let not_found = || HttpError { status: 404, message: "not found".into() };
        let root = self.web_dir.as_ref().ok_or_else(not_found)?;
        let rel = if path == "/" { "index.html" } else { path.trim_start_matches('/') };
        let rel_path = Path::new(rel);
        if rel.contains('\\') || !rel_path.components().all(|c| matches!(c, Component::Normal(_))) {
            return Err(not_found());
        }
        let full = root.join(rel_path);
        let body = std::fs::read(&full).map_err(|_| not_found())?;
        Ok(Response { status: 200, content_type: content_type(&full), body })
    }
}

fn upload_ext(name: &str) -> String {
    Path::new(name).extension().and_then(|e| e.to_str()).filter(|e| e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric())).unwrap_or("bin").to_ascii_lowercase()
}

fn job_spec(req: &Request) -> Result<JobSpec, HttpError> {
    let bad = |m: &str| HttpError { status: 400, message: m.to_string() };
    let bpm: f64 = req.param("bpm").and_then(|v| v.parse().ok()).ok_or_else(|| bad("bpm is required"))?;
    if !(bpm.is_finite() && bpm > 0.0 && bpm <= MAX_BPM) {
        return Err(bad("bpm must be between 0 and 400"));
    }
    let grid = match req.param("grid").unwrap_or("16") {
        "16" => 16,
        "24" => 24,
        _ => return Err(bad("grid must be 16 or 24")),
    };
    let separation = match req.param("separation").unwrap_or("none") {
        "none" => Separation::None,
        "vocals" => Separation::Vocals,
        "lead" => Separation::Lead,
        _ => return Err(bad("separation must be none, vocals or lead")),
    };
    let engine = match req.param("engine").unwrap_or("pyin") {
        "pyin" => Engine::Pyin,
        "game" => Engine::Game,
        "cells" => Engine::Cells,
        _ => return Err(bad("engine must be pyin, game or cells")),
    };
    let name: String = req.param("name").unwrap_or("upload.wav").chars().filter(|c| !c.is_control()).take(200).collect();
    Ok(JobSpec { name, bpm, grid, separation, engine })
}

fn job_status(job: &Job) -> serde_json::Value {
    let phase = job.phase();
    serde_json::json!({
        "id": job.id,
        "name": job.spec.name,
        "phase": phase.label(),
        "error": match &phase { Phase::Failed(e) => Some(e.clone()), _ => None },
        "elapsed_s": job.created.elapsed().as_secs_f64(),
        "log": job.log_tail(12).into_iter().chain(job.tool_line()).collect::<Vec<_>>(),
    })
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "wav" => "audio/wav",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}
