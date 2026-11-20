//! Finding or downloading the pinned gws binary.

use std::path::PathBuf;
use std::sync::Arc;

use haulmer_googleworkspace_mcp_core::gws_binary::{RELEASE_URL, release_asset};
use haulmer_googleworkspace_mcp_core::testing::{FakeHttp, FakeReply, FakeRunner, HttpRequest};
use haulmer_googleworkspace_mcp_core::{Core, CoreError, HttpResponse, Paths};
use sha2::{Digest, Sha256};

struct Setup {
    _home: tempfile::TempDir,
    app_data: tempfile::TempDir,
    runner: Arc<FakeRunner>,
    http: Arc<FakeHttp>,
    core: Core,
}

fn setup() -> Setup {
    let home = tempfile::tempdir().unwrap();
    let app_data = tempfile::tempdir().unwrap();
    let runner = Arc::new(FakeRunner::new());
    let http = Arc::new(FakeHttp::new());
    let paths = Paths {
        home: home.path().into(),
        app_data: app_data.path().into(),
    };
    let core = Core::new(paths, runner.clone(), http.clone());
    Setup {
        _home: home,
        app_data,
        runner,
        http,
        core,
    }
}

const PINNED: &str = "gws 0.22.5\nThis is not an officially supported Google product.\n";

#[test]
fn pinned_gws_on_path_is_used_without_any_download() {
    let s = setup();
    s.runner
        .on_program("gws", &["--version"], FakeReply::ok(PINNED));

    assert_eq!(s.core.find_gws().unwrap(), Some(PathBuf::from("gws")));
    assert_eq!(s.core.ensure_gws().unwrap(), PathBuf::from("gws"));
    assert!(s.http.requests().is_empty());
}

fn app_data_gws(s: &Setup) -> PathBuf {
    let name = if cfg!(windows) { "gws.exe" } else { "gws" };
    s.app_data.path().join("bin").join(name)
}

#[test]
fn pinned_gws_in_app_data_is_used_when_path_has_none() {
    let s = setup();
    let gws = app_data_gws(&s);
    std::fs::create_dir_all(gws.parent().unwrap()).unwrap();
    std::fs::write(&gws, "binary").unwrap();
    s.runner
        .on_program(&gws, &["--version"], FakeReply::ok(PINNED));

    assert_eq!(s.core.find_gws().unwrap(), Some(gws.clone()));
    assert_eq!(s.core.ensure_gws().unwrap(), gws);
    assert!(s.http.requests().is_empty());
}

#[test]
fn no_pinned_gws_anywhere_is_none_without_any_download() {
    let s = setup();
    s.runner
        .on_program("gws", &["--version"], FakeReply::ok("gws 0.21.0\n"));

    assert_eq!(s.core.find_gws().unwrap(), None);
    assert!(s.http.requests().is_empty());
}

#[cfg(unix)]
#[test]
fn pinned_gws_in_a_well_known_user_folder_is_found_when_path_has_none() {
    let s = setup();
    let home = s._home.path();
    let cargo = home.join(".cargo/bin/gws");
    let local = home.join(".local/bin/gws");
    for gws in [&cargo, &local] {
        std::fs::create_dir_all(gws.parent().unwrap()).unwrap();
        std::fs::write(gws, "binary").unwrap();
    }
    s.runner
        .on_program(&cargo, &["--version"], FakeReply::ok("gws 0.21.0\n"));
    s.runner
        .on_program(&local, &["--version"], FakeReply::ok(PINNED));

    assert_eq!(s.core.find_gws().unwrap(), Some(local));
    assert!(s.http.requests().is_empty());
}

#[cfg(unix)]
#[test]
fn well_known_folders_without_a_gws_file_are_not_probed() {
    let s = setup();

    assert_eq!(s.core.find_gws().unwrap(), None);

    let home = s._home.path();
    let probed: Vec<PathBuf> = s
        .runner
        .calls()
        .into_iter()
        .map(|c| c.command.program)
        .collect();
    assert!(!probed.contains(&home.join(".cargo/bin/gws")), "{probed:?}");
    assert!(!probed.contains(&home.join(".local/bin/gws")), "{probed:?}");
}

#[test]
fn version_probe_runs_in_the_controlled_gws_environment() {
    let s = setup();
    s.runner
        .on_program("gws", &["--version"], FakeReply::ok(PINNED));

    s.core.find_gws().unwrap();

    let probe = &s.runner.calls()[0].command;
    assert_eq!(probe.args, ["--version"]);
    assert_eq!(probe.cwd, s.app_data.path().join("run"));
    assert_eq!(probe.env["NO_COLOR"], "1");
    assert!(probe.env_remove.contains("GOOGLE_APPLICATION_CREDENTIALS"));
    assert!(probe.env_remove.contains("GOOGLE_WORKSPACE_CLI_TOKEN"));
}

const BINARY: &[u8] = b"#!fake gws binary";

/// A release archive with the binary and a LICENSE at its root, in this platform's format.
fn archive(asset: &str, binary: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let name = if cfg!(windows) { "gws.exe" } else { "gws" };
    if asset.ends_with(".zip") {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("LICENSE", options).unwrap();
        zip.write_all(b"Apache-2.0").unwrap();
        zip.start_file(name, options).unwrap();
        zip.write_all(binary).unwrap();
        zip.finish().unwrap().into_inner()
    } else {
        let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut tar = tar::Builder::new(gz);
        for (path, data) in [
            ("./LICENSE", b"Apache-2.0".as_slice()),
            (&format!("./{name}"), binary),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, path, data).unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap()
    }
}

/// The published checksum line, CRLF-terminated like the Windows file.
fn checksum_file(of: &[u8], asset: &str) -> String {
    let hex: String = Sha256::digest(of)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("{hex}  {asset}\r\n")
}

fn urls() -> (String, String) {
    let asset = release_asset().unwrap();
    (
        format!("{RELEASE_URL}/{asset}"),
        format!("{RELEASE_URL}/{asset}.sha256"),
    )
}

#[test]
fn release_url_is_the_pinned_github_release() {
    assert_eq!(
        RELEASE_URL,
        "https://github.com/googleworkspace/cli/releases/download/v0.22.5"
    );
}

#[test]
fn wrong_version_on_path_downloads_verifies_and_installs_the_pinned_gws() {
    let s = setup();
    s.runner
        .on_program("gws", &["--version"], FakeReply::ok("gws 0.21.0\n"));
    let asset = release_asset().unwrap();
    let (asset_url, checksum_url) = urls();
    let archive = archive(asset, BINARY);
    s.http.on(
        &checksum_url,
        HttpResponse::ok(checksum_file(&archive, asset)),
    );
    s.http.on(&asset_url, HttpResponse::ok(archive));

    let gws = s.core.ensure_gws().unwrap();

    assert_eq!(gws, app_data_gws(&s));
    assert_eq!(std::fs::read(&gws).unwrap(), BINARY);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&gws).unwrap().permissions().mode() & 0o111,
            0o111
        );
    }
    let mut urls: Vec<String> = s
        .http
        .requests()
        .into_iter()
        .map(|request| match request {
            HttpRequest::Get { url } => url,
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    urls.sort();
    assert_eq!(urls, [asset_url, checksum_url]);
}

#[test]
fn checksum_mismatch_is_an_error_and_leaves_no_binary_to_run() {
    let s = setup();
    let asset = release_asset().unwrap();
    let (asset_url, checksum_url) = urls();
    let archive = archive(asset, BINARY);
    let other = checksum_file(b"something else", asset);
    s.http.on(&checksum_url, HttpResponse::ok(other.clone()));
    s.http.on(&asset_url, HttpResponse::ok(archive));

    let result = s.core.ensure_gws();

    let expected = other.split_whitespace().next().unwrap().to_string();
    assert!(
        matches!(&result, Err(CoreError::ChecksumMismatch { expected: e, .. }) if *e == expected),
        "{result:?}"
    );
    let gws = app_data_gws(&s);
    assert!(!gws.exists());
    assert!(
        !gws.parent().unwrap().exists()
            || std::fs::read_dir(gws.parent().unwrap())
                .unwrap()
                .next()
                .is_none()
    );
    assert!(
        s.runner
            .calls()
            .iter()
            .all(|call| call.command.program != gws)
    );
}

#[test]
fn download_failures_are_clear_errors_and_write_nothing() {
    let s = setup();
    let (asset_url, checksum_url) = urls();
    s.http
        .on(&checksum_url, HttpResponse::ok(checksum_file(b"x", "a")));
    s.http.on(&asset_url, HttpResponse::new(404, "Not Found"));
    let not_found = s.core.ensure_gws();

    let offline = setup();
    offline
        .http
        .on_error(&checksum_url, CoreError::Http("offline".into()));
    let no_network = offline.core.ensure_gws();

    assert!(
        matches!(&not_found, Err(CoreError::Download(m)) if m.contains("404")),
        "{not_found:?}"
    );
    assert!(
        matches!(&no_network, Err(CoreError::Download(m)) if m.contains("offline")),
        "{no_network:?}"
    );
    assert!(!app_data_gws(&s).exists());
    assert!(!app_data_gws(&offline).exists());
}

#[test]
fn corrupt_archive_is_a_download_error() {
    let s = setup();
    let asset = release_asset().unwrap();
    let (asset_url, checksum_url) = urls();
    let mut archive = archive(asset, BINARY);
    archive.truncate(archive.len() / 2);
    s.http.on(
        &checksum_url,
        HttpResponse::ok(checksum_file(&archive, asset)),
    );
    s.http.on(&asset_url, HttpResponse::ok(archive));

    let result = s.core.ensure_gws();

    assert!(matches!(result, Err(CoreError::Download(_))), "{result:?}");
    assert!(!app_data_gws(&s).exists());
}

/// Serves one canned raw HTTP response per connection on localhost; returns the base URL
/// and a handle yielding the raw requests received.
fn local_server(responses: Vec<String>) -> (String, std::thread::JoinHandle<Vec<String>>) {
    use std::io::Write;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for response in responses {
            let (mut stream, _) = listener.accept().unwrap();
            requests.push(read_request(&mut stream));
            stream.write_all(response.as_bytes()).unwrap();
        }
        requests
    });
    (base, handle)
}

/// Reads one request: headers, then `Content-Length` bytes of body.
fn read_request(stream: &mut impl std::io::Read) -> String {
    let mut request = Vec::new();
    let mut byte = [0u8; 1];
    while !request.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap() == 1 {
        request.push(byte[0]);
    }
    let head = String::from_utf8_lossy(&request).to_ascii_lowercase();
    let length: usize = head
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .map_or(0, |value| value.trim().parse().unwrap());
    let mut body = vec![0u8; length];
    stream.read_exact(&mut body).unwrap();
    request.extend(body);
    String::from_utf8_lossy(&request).into_owned()
}

fn raw(status: &str, headers: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[test]
fn system_http_follows_redirects_and_returns_error_statuses_as_responses() {
    use haulmer_googleworkspace_mcp_core::{HttpClient, SystemHttp};
    let (base, server) = local_server(vec![
        raw("302 Found", "Location: /asset\r\n", ""),
        raw("200 OK", "", "archive bytes"),
        raw("404 Not Found", "", "Not Found"),
        raw("400 Bad Request", "", "invalid_token"),
    ]);
    let http = SystemHttp::new();

    let redirected = http.get(&format!("{base}/download")).unwrap();
    let missing = http.get(&format!("{base}/missing")).unwrap();
    let posted = http
        .post_form(&format!("{base}/revoke"), &[("token", "1//a b")])
        .unwrap();
    let requests = server.join().unwrap();

    assert_eq!(
        (redirected.status, redirected.body.as_slice()),
        (200, b"archive bytes".as_slice())
    );
    assert_eq!(missing.status, 404);
    assert_eq!(posted.status, 400);
    assert!(requests[1].starts_with("GET /asset "));
    assert!(requests[3].starts_with("POST /revoke "));
    assert!(requests[3].contains("application/x-www-form-urlencoded"));
    assert!(
        requests[3].ends_with("token=1%2F%2Fa+b") || requests[3].ends_with("token=1%2F%2Fa%20b"),
        "{}",
        requests[3]
    );
}

#[test]
fn system_http_without_a_server_is_an_http_error() {
    use haulmer_googleworkspace_mcp_core::{HttpClient, SystemHttp};
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();

    let result = SystemHttp::new().get(&format!("http://127.0.0.1:{port}/"));

    assert!(matches!(result, Err(CoreError::Http(_))), "{result:?}");
}
