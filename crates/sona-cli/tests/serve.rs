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
    _dir: TempDir,
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
fn stub_separators(root: &Path, demucs_extra: &str) -> (PathBuf, PathBuf) {
    let demucs = root.join("demucs-venv");
    script(
        &demucs.join("bin/python"),
        &format!(
            "{demucs_extra}\nwhile [ \"$1\" != \"-o\" ]; do shift; done\nout=$2; in=$3\nmkdir -p \"$out/htdemucs/song\"\ncp \"$in\" \"$out/htdemucs/song/vocals.wav\""
        ),
    );
    let sep = root.join("separator-venv");
    script(&sep.join("bin/audio-separator"), "in=$1\nwhile [ \"$1\" != \"--output_dir\" ]; do shift; done\nmkdir -p \"$2\"\ncp \"$in\" \"$2/vocals_(Vocals)_UVR.wav\"");
    (demucs, sep)
}

fn start(tag: &str, demucs_extra: &str) -> Server {
    let dir = temp_dir(tag);
    let (demucs, sep) = stub_separators(&dir.0, demucs_extra);
    let mut child = Command::new(env!("CARGO_BIN_EXE_sona"))
        .args(["serve", "--port", "0", "--web-dir"])
        .arg(&dir.0)
        .arg("--jobs-dir")
        .arg(dir.0.join("jobs"))
        .arg("--demucs-venv")
        .arg(&demucs)
        .arg("--separator-venv")
        .arg(&sep)
        .stdout(Stdio::piped())
        .spawn()
        .expect("start sona serve");
    std::fs::write(dir.0.join("index.html"), "<!doctype html><title>sona</title>").unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let port = line.trim().rsplit(':').next().and_then(|p| p.parse().ok()).unwrap_or_else(|| panic!("no port in {line:?}"));
    Server { child, port, _dir: dir }
}

fn request(port: u16, method: &str, path: &str, host: Option<&str>, body: &[u8]) -> (u16, Vec<u8>) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let host = host.map(str::to_string).unwrap_or_else(|| format!("127.0.0.1:{port}"));
    write!(s, "{method} {path} HTTP/1.1\r\nHost: {host}\r\nContent-Length: {}\r\n\r\n", body.len()).unwrap();
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
    let spec = hound::WavSpec { channels: 2, sample_rate: 44_100, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut cursor = std::io::Cursor::new(Vec::new());
    let mut w = hound::WavWriter::new(&mut cursor, spec).unwrap();
    for (freq, secs) in [(0.0f32, 0.3f32), (440.0, 0.8), (0.0, 0.3), (494.0, 0.8), (0.0, 0.3)] {
        for i in 0..(secs * 44_100.0) as usize {
            let v = (0.4 * (std::f32::consts::TAU * freq * i as f32 / 44_100.0).sin() * i16::MAX as f32) as i16;
            w.write_sample(v).unwrap();
            w.write_sample(v).unwrap();
        }
    }
    w.finalize().unwrap();
    cursor.into_inner()
}

fn wait_final(port: u16, id: &str) -> serde_json::Value {
    let t0 = Instant::now();
    loop {
        let (_, s) = json(port, "GET", &format!("/api/jobs/{id}"), b"");
        if ["done", "failed", "cancelled"].contains(&s["phase"].as_str().unwrap_or("")) {
            return s;
        }
        assert!(t0.elapsed() < Duration::from_secs(120), "job never finished: {s}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn a_song_goes_through_lead_separation_to_an_annotation_and_stem() {
    let srv = start("lead", "");
    let (status, caps) = json(srv.port, "GET", "/api/capabilities", b"");
    assert_eq!(status, 200);
    assert_eq!(caps["separation"]["lead"], true);
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
    let (status, _) = json(s.port, "POST", "/api/tempo?name=x.mp3", b"not audio at all");
    assert_eq!(status, 422);
}
