#![cfg(unix)]

use sha2::Digest;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Server {
    child: Child,
    port: u16,
    dir: TempDir,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

struct TempDir(PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp_dir(tag: &str) -> TempDir {
    let d = std::env::temp_dir().join(format!("sona-serve-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    TempDir(d)
}

fn script(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("#!/bin/sh\nset -e\n{body}\n")).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// demucs: `python -m demucs -n htdemucs --two-stems vocals -o OUT IN` → OUT/htdemucs/<name>/vocals.wav.
/// audio-separator: `audio-separator IN … --output_dir OUT …` → OUT/<name>_(Vocals)_model.wav.
/// Each run adds a line to `demucs.calls` / `uvr.calls` beside the venvs.
fn stub_separators(root: &Path, demucs_extra: &str) -> (PathBuf, PathBuf) {
    let demucs = root.join("demucs-venv");
    let r = root.display();
    script(
        &demucs.join("bin/python"),
        &format!(
            "{demucs_extra}\nwhile [ \"$1\" != \"-o\" ]; do shift; done\necho run >> \"{r}/demucs.calls\"\nout=$2; in=$3\nmkdir -p \"$out/htdemucs/song\"\ncp \"$in\" \"$out/htdemucs/song/vocals.wav\""
        ),
    );
    let sep = root.join("separator-venv");
    script(
        &sep.join("bin/audio-separator"),
        &format!("in=$1\nwhile [ \"$1\" != \"--output_dir\" ]; do shift; done\necho run >> \"{r}/uvr.calls\"\nmkdir -p \"$2\"\ncp \"$in\" \"$2/vocals_(Vocals)_UVR.wav\""),
    );
    (demucs, sep)
}

fn calls(srv: &Server, tool: &str) -> usize {
    std::fs::read_to_string(srv.dir.0.join(format!("{tool}.calls"))).map_or(0, |t| t.lines().count())
}

fn start(tag: &str, demucs_extra: &str) -> Server {
    start_with(tag, demucs_extra, &[], &[])
}

/// `sona serve` on a free port, with stub separators and its cache (jobs and stems) in a fresh folder.
/// Its ffmpeg is the one on `PATH`; where there is none, `-version` alone answers, so the start-up probe
/// passes and the tests that need a real ffmpeg skip themselves as before.
fn start_with(tag: &str, demucs_extra: &str, args: &[&str], env: &[(&str, &str)]) -> Server {
    let dir = temp_dir(tag);
    let (demucs, sep) = stub_separators(&dir.0, demucs_extra);
    let ffmpeg = dir.0.join("bin/ffmpeg-or-stub");
    script(&ffmpeg, "if [ \"$1\" = -version ] && ! command -v ffmpeg >/dev/null 2>&1; then echo 'ffmpeg version stub'; exit 0; fi\nexec ffmpeg \"$@\"");
    let mut child = Command::new(env!("CARGO_BIN_EXE_sona"))
        .args(["serve", "--port", "0", "--web-dir"])
        .arg(&dir.0)
        .arg("--cache-dir")
        .arg(&dir.0)
        .arg("--demucs-venv")
        .arg(&demucs)
        .arg("--separator-venv")
        .arg(&sep)
        .args(args)
        .env("SONA_FFMPEG", &ffmpeg)
        .envs(env.iter().copied())
        .stdout(Stdio::piped())
        .spawn()
        .expect("start sona serve");
    std::fs::write(dir.0.join("index.html"), "<!doctype html><title>sona</title>").unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let port = line.trim().rsplit(':').next().and_then(|p| p.parse().ok()).unwrap_or_else(|| panic!("no port in {line:?}"));
    Server { child, port, dir }
}

fn request(port: u16, method: &str, path: &str, host: Option<&str>, body: &[u8]) -> (u16, Vec<u8>) {
    let host = host.map(str::to_string).unwrap_or_else(|| format!("127.0.0.1:{port}"));
    request_with(port, method, path, &format!("Host: {host}\r\n"), body)
}

fn request_with(port: u16, method: &str, path: &str, headers: &str, body: &[u8]) -> (u16, Vec<u8>) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(s, "{method} {path} HTTP/1.1\r\n{headers}Content-Length: {}\r\n\r\n", body.len()).unwrap();
    s.write_all(body).unwrap();
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).unwrap();
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let status = std::str::from_utf8(&raw[9..12]).unwrap().parse().unwrap();
    (status, raw[split + 4..].to_vec())
}

fn json(port: u16, method: &str, path: &str, body: &[u8]) -> (u16, serde_json::Value) {
    let (status, b) = request(port, method, path, None, body);
    (status, serde_json::from_slice(&b).unwrap_or(serde_json::Value::Null))
}

fn song_wav() -> Vec<u8> {
    song_wav_at(44_100)
}

/// A4 then B4, stereo, at `rate`.
fn song_wav_at(rate: u32) -> Vec<u8> {
    let spec = hound::WavSpec { channels: 2, sample_rate: rate, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut cursor = std::io::Cursor::new(Vec::new());
    let mut w = hound::WavWriter::new(&mut cursor, spec).unwrap();
    for (freq, secs) in [(0.0f32, 0.3f32), (440.0, 0.8), (0.0, 0.3), (494.0, 0.8), (0.0, 0.3)] {
        for i in 0..(secs * rate as f32) as usize {
            let v = (0.4 * (std::f32::consts::TAU * freq * i as f32 / rate as f32).sin() * i16::MAX as f32) as i16;
            w.write_sample(v).unwrap();
            w.write_sample(v).unwrap();
        }
    }
    w.finalize().unwrap();
    cursor.into_inner()
}

fn wait_final(port: u16, id: &str) -> serde_json::Value {
    wait_final_within(port, id, Duration::from_secs(120))
}

fn wait_final_within(port: u16, id: &str, limit: Duration) -> serde_json::Value {
    let t0 = Instant::now();
    loop {
        let (_, s) = json(port, "GET", &format!("/api/jobs/{id}"), b"");
        if ["done", "failed", "cancelled"].contains(&s["phase"].as_str().unwrap_or("")) {
            return s;
        }
        assert!(t0.elapsed() < limit, "job never finished: {s}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn a_song_goes_through_lead_separation_to_an_annotation_and_stem() {
    let srv = start("lead", "command -v ffmpeg-or-stub >/dev/null || { echo 'the ffmpeg sona was given is not on the separator PATH' >&2; exit 3; }");
    let (status, caps) = json(srv.port, "GET", "/api/capabilities", b"");
    assert_eq!(status, 200);
    assert_eq!(caps["separation"]["lead"], true, "a lead model that isn't downloaded yet doesn't turn lead off: {caps}");
    let model = setup_note(&caps, "lead-model").expect("the stub venv has no model file");
    assert_eq!(model["status"], "first-use");
    assert!(model["message"].as_str().unwrap().contains("UVR_MDXNET_KARA_2.onnx"), "{model}");
    let (status, job) = json(srv.port, "POST", "/api/jobs?bpm=120&grid=16&separation=lead&engine=pyin&name=song.wav", &song_wav());
    assert_eq!(status, 201, "{job}");
    let id = job["id"].as_str().unwrap();
    let fin = wait_final(srv.port, id);
    assert_eq!(fin["phase"], "done", "{fin}");
    let (status, ann) = json(srv.port, "GET", &format!("/api/jobs/{id}/annotation"), b"");
    assert_eq!(status, 200);
    let pitches: Vec<i64> = ann["notes"].as_array().unwrap().iter().map(|n| n["midi_pitch"].as_i64().unwrap()).collect();
    assert!(pitches.contains(&69) && pitches.contains(&71), "A4 and B4 expected, got {pitches:?}");
    let (status, stem) = request(srv.port, "GET", &format!("/api/jobs/{id}/stem"), None, b"");
    assert_eq!(status, 200);
    let reader = hound::WavReader::new(std::io::Cursor::new(stem.clone())).unwrap();
    assert_eq!((reader.spec().channels, reader.spec().sample_rate), (1, 44_100), "the stem is mono at the song's rate");

    let sha = |b: &[u8]| -> String { sha2::Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect() };
    let p = &ann["provenance"];
    assert_eq!(p["input"]["sha256"], sha(&stem), "the annotation names the exact stem it came from");
    assert_eq!(p["settings"]["engine"], "pyin");
    assert_eq!(p["settings"]["grid"], "16");
    assert_eq!(p["job"]["upload"]["sha256"], sha(&song_wav()), "and the exact upload");
    assert_eq!(p["job"]["upload"]["name"], "song.wav");
    let steps: Vec<&str> = p["job"]["steps"].as_array().unwrap().iter().map(|s| s["step"].as_str().unwrap()).collect();
    assert_eq!(steps, ["queued", "separating-vocals", "separating-lead", "transcribing"]);
    assert!(p["job"]["steps"].as_array().unwrap().iter().all(|s| s["seconds"].as_f64().unwrap() >= 0.0));
    assert_eq!(p["job"]["tools"]["demucs"], "unknown", "the stub has no demucs package, and says so rather than guessing");
    let (status, record) = json(srv.port, "GET", &format!("/api/jobs/{id}/provenance"), b"");
    assert_eq!(status, 200);
    assert_eq!(record["outcome"], "done");
    assert_eq!(record["job"]["upload"], p["job"]["upload"]);
}

#[test]
fn a_failing_separator_fails_the_job_with_its_message() {
    let srv = start("fail", "echo 'demucs: out of memory' >&2; exit 3");
    let (_, job) = json(srv.port, "POST", "/api/jobs?bpm=120&separation=vocals&name=song.wav", &song_wav());
    let fin = wait_final(srv.port, job["id"].as_str().unwrap());
    assert_eq!(fin["phase"], "failed");
    assert!(fin["error"].as_str().unwrap().contains("out of memory"), "{fin}");
    let (status, _) = json(srv.port, "GET", &format!("/api/jobs/{}/annotation", job["id"].as_str().unwrap()), b"");
    assert_eq!(status, 409, "no annotation from a failed job");
    let (status, record) = json(srv.port, "GET", &format!("/api/jobs/{}/provenance", job["id"].as_str().unwrap()), b"");
    assert_eq!(status, 200, "a failed job still leaves its record");
    assert_eq!(record["outcome"], "failed");
    assert!(record["error"].as_str().unwrap().contains("out of memory"), "{record}");
    let steps: Vec<&str> = record["job"]["steps"].as_array().unwrap().iter().map(|s| s["step"].as_str().unwrap()).collect();
    assert_eq!(steps, ["queued", "separating-vocals"]);
}

#[test]
fn cancelling_stops_a_running_separator_and_never_reports_done() {
    let srv = start("cancel", "sleep 30");
    let (_, job) = json(srv.port, "POST", "/api/jobs?bpm=120&separation=vocals&name=song.wav", &song_wav());
    let id = job["id"].as_str().unwrap().to_string();
    std::thread::sleep(Duration::from_millis(400));
    let t0 = Instant::now();
    let (status, _) = json(srv.port, "DELETE", &format!("/api/jobs/{id}"), b"");
    assert_eq!(status, 202);
    let fin = wait_final(srv.port, &id);
    assert_eq!(fin["phase"], "cancelled");
    assert!(t0.elapsed() < Duration::from_secs(5), "the separator was killed, not waited for");
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(json(srv.port, "GET", &format!("/api/jobs/{id}"), b"").1["phase"], "cancelled");
    let t1 = Instant::now();
    let (_, next) = json(srv.port, "POST", "/api/jobs?bpm=120&separation=none&name=song.wav", &song_wav());
    assert_eq!(wait_final(srv.port, next["id"].as_str().unwrap())["phase"], "done");
    assert!(t1.elapsed() < Duration::from_secs(10), "the worker was still busy with the cancelled separator");
}

#[test]
fn refuses_foreign_hosts_bad_input_and_paths_outside_the_web_build() {
    let srv = start("guard", "");
    assert_eq!(request(srv.port, "GET", "/api/capabilities", Some("evil.example:8977"), b"").0, 403, "DNS-rebinding host");
    let host = format!("Host: 127.0.0.1:{}\r\n", srv.port);
    let from = |origin: &str| request_with(srv.port, "POST", "/api/jobs?bpm=0&name=a.wav", &format!("{host}Origin: {origin}\r\n"), b"x").0;
    assert_eq!(from("https://evil.example"), 403, "a cross-origin simple POST from another site");
    assert_eq!(from("null"), 403);
    assert_eq!(from(&format!("http://localhost:{}", srv.port)), 400, "the server's own page gets as far as the bad bpm");
    assert_eq!(from(&format!("http://127.0.0.1:{}", srv.port)), 400);
    assert_eq!(json(srv.port, "POST", "/api/jobs?bpm=0&name=a.wav", b"x").0, 400);
    assert_eq!(json(srv.port, "POST", "/api/jobs?bpm=120&grid=7&name=a.wav", b"x").0, 400);
    assert_eq!(json(srv.port, "POST", "/api/jobs?bpm=120&engine=game&name=a.wav", b"x").0, 409, "GAME not configured");
    assert_eq!(json(srv.port, "POST", "/api/jobs?bpm=120&name=a.wav", b"").0, 400, "empty upload");
    assert_eq!(request(srv.port, "GET", "/../../etc/passwd", None, b"").0, 404);
    assert_eq!(request(srv.port, "GET", "/%2e%2e/%2e%2e/etc/passwd", None, b"").0, 404);
    assert_eq!(request(srv.port, "GET", "/api/jobs/nope", None, b"").0, 404);
    let (status, page) = request(srv.port, "GET", "/", None, b"");
    assert_eq!((status, String::from_utf8_lossy(&page).contains("<title>sona</title>")), (200, true));
}

fn click_wav(bpm: f64, seconds: f64) -> Vec<u8> {
    let sr = 22_050u32;
    let spec = hound::WavSpec { channels: 1, sample_rate: sr, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut cursor = std::io::Cursor::new(Vec::new());
    let mut w = hound::WavWriter::new(&mut cursor, spec).unwrap();
    let eighth = 30.0 / bpm;
    for i in 0..(seconds * f64::from(sr)) as usize {
        let t = i as f64 / f64::from(sr);
        let since = t % eighth;
        let beat = ((t / eighth).floor() as i64) % 2 == 0;
        let f = if beat { 60.0 } else { 7000.0 };
        let v = if since < 0.03 { (-since * 120.0).exp() * (std::f64::consts::TAU * f * t).sin() * if beat { 0.8 } else { 0.3 } } else { 0.0 };
        w.write_sample((v * f64::from(i16::MAX)) as i16).unwrap();
    }
    w.finalize().unwrap();
    cursor.into_inner()
}

#[test]
fn estimates_a_songs_tempo_from_wav_and_through_ffmpeg() {
    let s = start("tempo", "");
    let (status, caps) = json(s.port, "GET", "/api/capabilities", b"");
    assert_eq!(status, 200);
    if caps["tempo"] != true {
        return;
    }
    let wav = click_wav(180.0, 25.0);
    let (status, body) = json(s.port, "POST", "/api/tempo?name=clicks.wav", &wav);
    assert_eq!(status, 200, "{body}");
    let bpm = body["bpm"].as_f64().unwrap();
    assert!((bpm - 180.0).abs() / 180.0 <= 0.02, "{bpm}");

    if std::process::Command::new("ffmpeg").arg("-version").output().is_ok() {
        let dir = temp_dir("tempo-flac");
        let src = dir.0.join("clicks.wav");
        let flac = dir.0.join("clicks.flac");
        std::fs::write(&src, &wav).unwrap();
        assert!(std::process::Command::new("ffmpeg").args(["-nostdin", "-loglevel", "error", "-y", "-i"]).arg(&src).arg(&flac).status().unwrap().success());
        let (status, body) = json(s.port, "POST", "/api/tempo?name=clicks.flac", &std::fs::read(&flac).unwrap());
        assert_eq!(status, 200, "{body}");
        assert!((body["bpm"].as_f64().unwrap() - 180.0).abs() / 180.0 <= 0.02, "{body}");
    }

    assert_eq!(caps["engines"]["cells"], false, "no cell model on this server");
    let (status, body) = json(s.port, "POST", "/api/jobs?bpm=120&separation=lead&engine=cells&name=song.wav", &click_wav(120.0, 2.0));
    assert_eq!(status, 409, "{body}");
    assert!(body["error"].as_str().unwrap().contains("--cell-model"), "{body}");
    let (status, _) = json(s.port, "POST", "/api/tempo?name=x.mp3", b"");
    assert_eq!(status, 400);
    let (status, body) = json(s.port, "POST", "/api/tempo?name=x.mp3", b"not audio at all");
    assert_eq!(status, 422);
    let error = body["error"].as_str().unwrap();
    assert!(error.starts_with("ffmpeg could not read the file: ") && !error.contains("/scratch/") && !error.contains('\n'), "one line, without the server's scratch path: {error}");
}

fn submit(port: u16, query: &str, audio: &[u8]) -> String {
    let (status, job) = json(port, "POST", &format!("/api/jobs?{query}"), audio);
    assert_eq!(status, 201, "{job}");
    job["id"].as_str().unwrap().to_string()
}

fn annotation(port: u16, id: &str) -> serde_json::Value {
    let (status, ann) = json(port, "GET", &format!("/api/jobs/{id}/annotation"), b"");
    assert_eq!(status, 200, "{ann}");
    ann
}

fn steps(ann: &serde_json::Value) -> Vec<String> {
    ann["provenance"]["job"]["steps"].as_array().unwrap().iter().map(|s| s["step"].as_str().unwrap().to_string()).collect()
}

#[test]
fn transcribing_a_song_again_reuses_its_separated_stems() {
    let srv = start("cache", "");
    let song = song_wav();
    let first = submit(srv.port, "bpm=120&separation=lead&name=song.wav", &song);
    assert_eq!(wait_final(srv.port, &first)["phase"], "done");
    let again = submit(srv.port, "bpm=120&separation=lead&name=song.wav", &song);
    let fin = wait_final(srv.port, &again);
    assert_eq!(fin["phase"], "done", "{fin}");
    let half = submit(srv.port, "bpm=60&grid=24&separation=lead&name=song.wav", &song);
    assert_eq!(wait_final(srv.port, &half)["phase"], "done");
    assert_eq!((calls(&srv, "demucs"), calls(&srv, "uvr")), (1, 1), "both separators ran once for three jobs");

    let (a, b) = (annotation(srv.port, &first), annotation(srv.port, &again));
    assert_eq!(a["notes"], b["notes"], "the same stem gives the same notes");
    assert_eq!(request(srv.port, "GET", &format!("/api/jobs/{first}/stem"), None, b"").1, request(srv.port, "GET", &format!("/api/jobs/{again}/stem"), None, b"").1);
    let (made, reused) = (&a["provenance"]["job"]["stems"], &b["provenance"]["job"]["stems"]);
    assert_eq!((made["reused"].as_bool(), made["separated_by"].as_str()), (Some(false), Some(first.as_str())));
    assert_eq!((reused["reused"].as_bool(), reused["separated_by"].as_str()), (Some(true), Some(first.as_str())));
    assert_eq!(made["key"], reused["key"]);
    assert_eq!(made["key"].as_str().map(str::len), Some(64));
    assert_eq!(steps(&a), ["queued", "separating-vocals", "separating-lead", "transcribing"]);
    assert_eq!(steps(&b), ["queued", "reused-stems", "transcribing"]);
    assert!(fin["log"].as_array().unwrap().iter().any(|l| l.as_str().unwrap().contains(&format!("separation reused from job {first}"))), "{fin}");
    let h = annotation(srv.port, &half);
    assert_eq!((h["bpm"].as_f64(), h["provenance"]["settings"]["grid"].as_str()), (Some(60.0), Some("24")));
    assert_eq!(h["provenance"]["job"]["stems"]["reused"], true);

    let vocals_only = submit(srv.port, "bpm=120&separation=vocals&name=song.wav", &song);
    assert_eq!(wait_final(srv.port, &vocals_only)["phase"], "done");
    let mut other = song.clone();
    other.extend_from_slice(&[0, 0, 0, 0]);
    let other = submit(srv.port, "bpm=120&separation=lead&name=song.wav", &other);
    assert_eq!(wait_final(srv.port, &other)["phase"], "done");
    assert_eq!(calls(&srv, "demucs"), 3, "another separation or another upload separates again");
    for id in [&first, &other] {
        let dir = srv.dir.0.join("jobs").join(id);
        assert!(dir.join("stem.wav").is_file() && !dir.join("demucs").exists() && !dir.join("uvr").exists(), "the separators' working folders go once the stem is out");
    }
}

#[test]
fn old_jobs_and_stems_are_deleted_but_never_a_queued_one() {
    let srv = start_with("tidy", "case \"$*\" in *htdemucs*) sleep 1;; esac", &["--keep-jobs", "1", "--keep-stems", "1"], &[]);
    let jobs = srv.dir.0.join("jobs");
    for (name, log) in [("0abc", true), ("notes", true), ("0def", false)] {
        std::fs::create_dir_all(jobs.join(name)).unwrap();
        if log {
            std::fs::write(jobs.join(name).join("job.log"), "[ 0.0s] received 1 bytes\n").unwrap();
        }
    }
    let mut b_song = song_wav();
    b_song.extend_from_slice(&[0, 0, 0, 0]);
    let a = submit(srv.port, "bpm=120&separation=vocals&name=a.wav", &song_wav());
    let b = submit(srv.port, "bpm=120&separation=vocals&name=b.wav", &b_song);
    let c = submit(srv.port, "bpm=120&separation=none&name=c.wav", &song_wav());
    for id in [&a, &b, &c] {
        let fin = wait_final(srv.port, id);
        assert_eq!(fin["phase"], "done", "a queued job keeps its folder: {fin}");
    }
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(json(srv.port, "GET", &format!("/api/jobs/{a}"), b"").0, 404, "the oldest job is gone");
    assert_eq!(json(srv.port, "GET", &format!("/api/jobs/{b}"), b"").0, 404);
    assert_eq!(annotation(srv.port, &c)["notes"].as_array().map(|n| !n.is_empty()), Some(true), "the newest is kept");
    let mut left: Vec<String> = std::fs::read_dir(&jobs).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    left.sort();
    assert_eq!(left, vec!["0def".to_string(), c.clone(), "notes".to_string()], "only job folders (hex, with a job log) are ever deleted");
    let stems = std::fs::read_dir(srv.dir.0.join("stems")).unwrap().count();
    assert_eq!(stems, 1, "the stem cache keeps one song");
}

#[test]
fn a_panic_fails_that_job_and_the_worker_goes_on() {
    let srv = start_with("panic", "", &[], &[("SONA_TEST_PANIC", "boom.wav")]);
    let boom = submit(srv.port, "bpm=120&separation=none&name=boom.wav", &song_wav());
    let next = submit(srv.port, "bpm=120&separation=none&name=song.wav", &song_wav());
    let fin = wait_final_within(srv.port, &boom, Duration::from_secs(30));
    assert_eq!(fin["phase"], "failed", "{fin}");
    let error = fin["error"].as_str().unwrap();
    assert!(error.contains("internal error") && error.contains("asked for a panic") && error.contains("panic.log"), "{error}");
    let report = std::fs::read_to_string(srv.dir.0.join("jobs").join(&boom).join("panic.log")).unwrap();
    assert!(report.contains("asked for a panic in boom.wav"), "{report}");
    let (status, record) = json(srv.port, "GET", &format!("/api/jobs/{boom}/provenance"), b"");
    assert_eq!((status, record["outcome"].as_str()), (200, Some("failed")));
    assert_eq!(wait_final_within(srv.port, &next, Duration::from_secs(30))["phase"], "done", "the worker survived");
    let after = submit(srv.port, "bpm=120&separation=none&name=song.wav", &song_wav());
    assert_eq!(wait_final_within(srv.port, &after, Duration::from_secs(30))["phase"], "done");
}

#[test]
fn a_hung_separator_is_stopped_at_its_time_limit_and_shows_its_progress() {
    let hang = "case \"$*\" in *htdemucs*) printf ' 40%%|####      | 4/10 [00:02<00:03]\\r'; sleep 30;; esac";
    let srv = start_with("hang", hang, &["--tool-timeout", "1", "--tool-timeout-ratio", "0.6"], &[]);
    let t0 = Instant::now();
    let id = submit(srv.port, "bpm=120&separation=vocals&name=song.wav", &song_wav());
    let mut progress = None;
    let fin = loop {
        let (_, s) = json(srv.port, "GET", &format!("/api/jobs/{id}"), b"");
        if let Some(line) = s["log"].as_array().unwrap().iter().filter_map(|l| l.as_str()).find(|l| l.starts_with("demucs: ")) {
            progress = Some(line.to_string());
        }
        if ["done", "failed", "cancelled"].contains(&s["phase"].as_str().unwrap_or("")) {
            break s;
        }
        assert!(t0.elapsed() < Duration::from_secs(10), "the hung separator held the job: {s}");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(progress.as_deref(), Some("demucs: 40%|####      | 4/10 [00:02<00:03]"), "the tool's last line shows while it runs");
    assert_eq!(fin["phase"], "failed");
    let error = fin["error"].as_str().unwrap();
    assert!(error.contains("demucs was stopped after 2 s, its time limit for") && error.contains("s of audio") && error.contains("4/10") && error.contains("--tool-timeout"), "{error}");
    let next = submit(srv.port, "bpm=120&separation=none&name=song.wav", &song_wav());
    assert_eq!(wait_final_within(srv.port, &next, Duration::from_secs(10))["phase"], "done", "the queue moved on");
}

#[test]
fn a_48_khz_wav_becomes_a_44_1_khz_stem() {
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        return;
    }
    let srv = start("rate", "");
    let id = submit(srv.port, "bpm=120&separation=none&engine=pyin&name=song48k.wav", &song_wav_at(48_000));
    assert_eq!(wait_final(srv.port, &id)["phase"], "done");
    let (_, stem) = request(srv.port, "GET", &format!("/api/jobs/{id}/stem"), None, b"");
    let reader = hound::WavReader::new(std::io::Cursor::new(stem)).unwrap();
    assert_eq!((reader.spec().channels, reader.spec().sample_rate), (1, 44_100));
    let pitches: Vec<i64> = annotation(srv.port, &id)["notes"].as_array().unwrap().iter().map(|n| n["midi_pitch"].as_i64().unwrap()).collect();
    assert!(pitches.contains(&69) && pitches.contains(&71), "A4 and B4 expected, got {pitches:?}");
}

#[test]
fn help_works_on_both_subcommands_and_a_missing_separator_says_what_to_do() {
    for args in [["serve", "--help"], ["transcribe", "-h"]] {
        let out = Command::new(env!("CARGO_BIN_EXE_sona")).args(args).output().unwrap();
        assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
        assert!(String::from_utf8_lossy(&out.stdout).contains("--keep-stems"), "{args:?}");
    }
    let srv = start_with("nosep", "", &["--demucs-venv", "/nonexistent/demucs-venv"], &[]);
    let (status, body) = json(srv.port, "POST", "/api/jobs?bpm=120&separation=lead&name=song.wav", &song_wav());
    assert_eq!(status, 409);
    let error = body["error"].as_str().unwrap();
    assert!(error.contains("setup-separators.sh") && error.contains("--demucs-venv") && error.contains("sona serve --help"), "{error}");
}

fn setup_note<'a>(caps: &'a serde_json::Value, piece: &str) -> Option<&'a serde_json::Value> {
    caps["setup"].as_array().unwrap().iter().find(|n| n["piece"] == piece)
}

#[test]
fn without_ffmpeg_separation_is_off_and_a_file_that_needs_it_is_refused_with_the_fix() {
    let srv = start_with("noffmpeg", "", &[], &[("SONA_FFMPEG", "/nonexistent/ffmpeg")]);
    let (_, caps) = json(srv.port, "GET", "/api/capabilities", b"");
    assert_eq!(caps["separation"], serde_json::json!({ "none": true, "vocals": false, "lead": false }), "{caps}");
    let note = setup_note(&caps, "ffmpeg").expect("ffmpeg is reported");
    assert_eq!((note["status"].as_str(), note["blocks"].clone()), (Some("missing"), serde_json::json!(["vocals", "lead"])));
    assert!(note["message"].as_str().unwrap().contains("install ffmpeg"), "{note}");
    for sep in ["vocals", "lead"] {
        let (status, body) = json(srv.port, "POST", &format!("/api/jobs?bpm=120&separation={sep}&name=song.wav"), &song_wav());
        assert_eq!(status, 409, "{body}");
        assert!(body["error"].as_str().unwrap().contains("install ffmpeg"), "{body}");
    }
    for (name, audio) in [("song48k.wav", song_wav_at(48_000)), ("song.mp3", b"ID3 not really an mp3".to_vec())] {
        let (status, body) = json(srv.port, "POST", &format!("/api/jobs?bpm=120&separation=none&name={name}"), &audio);
        assert_eq!(status, 409, "{name}: {body}");
        assert!(body["error"].as_str().unwrap().contains("install ffmpeg"), "{body}");
    }
    let id = submit(srv.port, "bpm=120&separation=none&name=song.wav", &song_wav());
    assert_eq!(wait_final(srv.port, &id)["phase"], "done", "a 44.1 kHz WAV needs no ffmpeg");
    if caps["tempo"] == true {
        let (status, body) = json(srv.port, "POST", "/api/tempo?name=x.mp3", b"ID3 not really an mp3");
        assert_eq!(status, 422);
        assert!(body["error"].as_str().unwrap().contains("install ffmpeg"), "{body}");
        assert_eq!(json(srv.port, "POST", "/api/tempo?name=clicks.wav", &click_wav(120.0, 6.0)).0, 200, "any WAV is read without ffmpeg");
    }
}

#[test]
fn a_missing_lead_separator_or_model_is_reported_with_its_fix() {
    let torch = temp_dir("torch-home");
    let srv = start_with("nolead", "", &["--separator-venv", "/nonexistent/separator-venv"], &[("TORCH_HOME", torch.0.to_str().unwrap())]);
    let (_, caps) = json(srv.port, "GET", "/api/capabilities", b"");
    assert_eq!((caps["separation"]["vocals"].as_bool(), caps["separation"]["lead"].as_bool()), (Some(true), Some(false)), "{caps}");
    let sep = setup_note(&caps, "separator").expect("the separator is reported");
    assert_eq!(sep["status"], "missing");
    assert_eq!(caps["setup"][0]["piece"], "separator", "what is missing comes before what downloads on first use");
    let message = sep["message"].as_str().unwrap();
    assert!(message.contains("setup-separators.sh lead") && message.contains("--separator-venv"), "{message}");
    assert!(setup_note(&caps, "lead-model").is_none(), "the missing venv covers its model");
    let weights = setup_note(&caps, "demucs-weights").expect("no weights in this TORCH_HOME");
    assert_eq!((weights["status"].as_str(), weights["blocks"].clone()), (Some("first-use"), serde_json::json!([])));

    let (status, body) = json(srv.port, "POST", "/api/jobs?bpm=120&separation=lead&name=song.wav", &song_wav());
    assert_eq!(status, 409);
    let error = body["error"].as_str().unwrap();
    assert!(error.contains("setup-separators.sh lead") && error.contains("--separator-venv") && error.contains("sona serve --help"), "{error}");

    let checkpoint = torch.0.join("hub/checkpoints/955717e8-8726e21a.th");
    std::fs::create_dir_all(checkpoint.parent().unwrap()).unwrap();
    std::fs::write(&checkpoint, b"weights").unwrap();
    let (_, caps) = json(srv.port, "GET", "/api/capabilities", b"");
    assert!(setup_note(&caps, "demucs-weights").is_none(), "weights that arrive while serving count at once: {caps}");
}
