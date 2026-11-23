use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::http::{read_request, write_response, HttpError, Limits, Request, Response};
use crate::jobs::{Engine, Job, JobSpec, Phase, Queue, Separation, Tools};

const LIMITS: Limits = Limits { max_header_bytes: 16 * 1024, max_body_bytes: 500 * 1_000_000 };
const DEFAULT_PORT: u16 = 8977;
const MAX_BPM: f64 = 400.0;

pub struct ServeArgs {
    port: u16,
    web_dir: Option<PathBuf>,
    jobs_dir: PathBuf,
    tools: Tools,
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

pub fn parse_serve_args(args: &[String]) -> Result<ServeArgs, String> {
    let share = home().join(".local/share/sona");
    let mut port = DEFAULT_PORT;
    let mut web_dir: Option<PathBuf> = None;
    let mut jobs_dir = std::env::temp_dir().join("sona-jobs");
    let mut demucs_venv = std::env::var_os("SONA_DEMUCS_VENV").map(PathBuf::from).unwrap_or_else(|| share.join("demucs-venv"));
    let mut separator_venv = std::env::var_os("SONA_LEAD_VENV").map(PathBuf::from).unwrap_or_else(|| share.join("separator-venv"));
    let mut game_dir: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        let value = |i: usize| args.get(i + 1).cloned().ok_or_else(|| format!("{} expects a value", args[i]));
        match args[i].as_str() {
            "--port" => port = value(i)?.parse().map_err(|_| "--port must be a number (0 picks a free port)")?,
            "--web-dir" => web_dir = Some(PathBuf::from(value(i)?)),
            "--jobs-dir" => jobs_dir = PathBuf::from(value(i)?),
            "--demucs-venv" => demucs_venv = PathBuf::from(value(i)?),
            "--separator-venv" => separator_venv = PathBuf::from(value(i)?),
            "--game" => game_dir = Some(PathBuf::from(value(i)?)),
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
    let tools = Tools {
        demucs_python: demucs_venv.join("bin/python"),
        separator_bin: separator_venv.join("bin/audio-separator"),
        separator_models: separator_venv.join("models"),
        lead_model: std::env::var("SONA_LEAD_MODEL").unwrap_or_else(|_| "UVR_MDXNET_KARA_2.onnx".into()),
        ffmpeg: PathBuf::from("ffmpeg"),
        game_dir,
    };
    Ok(ServeArgs { port, web_dir, jobs_dir, tools })
}

struct Server {
    port: u16,
    web_dir: Option<PathBuf>,
    tools: Tools,
    queue: Arc<Queue>,
}

pub fn run_serve(args: &[String]) -> Result<(), String> {
    let a = parse_serve_args(args)?;
    let listener = TcpListener::bind(("127.0.0.1", a.port)).map_err(|e| format!("binding 127.0.0.1:{}: {e}", a.port))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let queue = Queue::start(a.jobs_dir.clone(), a.tools.clone()).map_err(|e| format!("jobs dir {}: {e}", a.jobs_dir.display()))?;
    let server = Arc::new(Server { port, web_dir: a.web_dir, tools: a.tools, queue });
    println!("sona: serving http://127.0.0.1:{port}");
    if server.web_dir.is_none() {
        println!("sona: no web build found (run `npm run build` in web/, or pass --web-dir); the API still works");
    }
    use std::io::Write;
    let _ = std::io::stdout().flush();
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

    fn capabilities(&self) -> serde_json::Value {
        let demucs = self.tools.demucs_python.is_file();
        let lead = demucs && self.tools.separator_bin.is_file();
        let game = cfg!(feature = "game") && self.tools.game_dir.is_some() && std::env::var_os("ORT_DYLIB_PATH").is_some();
        serde_json::json!({
            "separation": { "none": true, "vocals": demucs, "lead": lead },
            "engines": { "pyin": true, "game": game },
            "tempo": cfg!(feature = "spectral"),
        })
    }

    fn route(&self, req: &Request) -> Result<Response, HttpError> {
        // A DNS-rebinding page reaches 127.0.0.1 under its own hostname, and the browser sends that name as `Host`.
        let host_ok = req.header("Host").is_some_and(|h| h == format!("127.0.0.1:{}", self.port) || h == format!("localhost:{}", self.port));
        if !host_ok {
            return Err(HttpError { status: 403, message: "this server only answers its own address".into() });
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

    fn create_job(&self, req: &Request) -> Result<Response, HttpError> {
        let spec = job_spec(req)?;
        let caps = self.capabilities();
        let sep_key = match spec.separation {
            Separation::None => "none",
            Separation::Vocals => "vocals",
            Separation::Lead => "lead",
        };
        if caps["separation"][sep_key] != true {
            return Err(HttpError { status: 409, message: format!("separation '{sep_key}' isn't set up on this server (see `sona serve --help`)") });
        }
        if spec.engine == Engine::Game && caps["engines"]["game"] != true {
            return Err(HttpError { status: 409, message: "GAME isn't set up on this server (start it with --game DIR and ORT_DYLIB_PATH)".into() });
        }
        if req.body.is_empty() {
            return Err(HttpError { status: 400, message: "send the audio file as the request body".into() });
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
        _ => return Err(bad("engine must be pyin or game")),
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
        "log": job.log_tail(12),
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
