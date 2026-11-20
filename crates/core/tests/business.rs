use std::sync::Arc;

use haulmer_googleworkspace_mcp_core::gws_env::write_marker;
use haulmer_googleworkspace_mcp_core::testing::{FakeHttp, FakeReply, FakeRunner, HttpRequest};
use haulmer_googleworkspace_mcp_core::{BusinessConnector, Core, CoreError, HttpResponse, Paths};
use tempfile::TempDir;

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const YOUTUBE_URL: &str = "https://www.googleapis.com/youtube/v3/channels?part=snippet,contentDetails,statistics&mine=true";

struct Fixture {
    _home: TempDir,
    _app_data: TempDir,
    runner: Arc<FakeRunner>,
    http: Arc<FakeHttp>,
    core: Core,
}

fn fixture() -> Fixture {
    let home = TempDir::new().unwrap();
    let app_data = TempDir::new().unwrap();
    let runner = Arc::new(FakeRunner::new());
    let http = Arc::new(FakeHttp::new());
    let paths = Paths {
        home: home.path().into(),
        app_data: app_data.path().into(),
    };
    let core = Core::new(paths, runner.clone(), http.clone());
    let config = home.path().join(".config/gws");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("credentials.enc"), b"encrypted").unwrap();
    write_marker(&config).unwrap();
    runner.on_program("gws", &["--version"], FakeReply::ok("gws 0.22.5\n"));
    Fixture {
        _home: home,
        _app_data: app_data,
        runner,
        http,
        core,
    }
}

#[test]
fn youtube_channel_refreshes_gws_credentials_and_uses_bearer_auth() {
    let fixture = fixture();
    fixture.runner.on(
        &["auth", "export", "--unmasked"],
        FakeReply::ok(r#"{"refresh_token":"refresh-token"}"#),
    );
    fixture.http.on(
        TOKEN_URL,
        HttpResponse::ok(r#"{"access_token":"access-token","expires_in":3600}"#),
    );
    fixture.http.on(
        YOUTUBE_URL,
        HttpResponse::ok(r#"{"items":[{"snippet":{"title":"Haulmer"}}]}"#),
    );

    let channel = fixture.core.youtube_channel().unwrap();

    assert_eq!(channel["items"][0]["snippet"]["title"], "Haulmer");
    assert!(fixture.http.requests().iter().any(|request| matches!(
        request,
        HttpRequest::PostForm { url, form }
            if url == TOKEN_URL
                && form.iter().any(|(key, value)| key == "refresh_token" && value == "refresh-token")
    )));
    assert!(fixture.http.requests().iter().any(|request| matches!(
        request,
        HttpRequest::GetBearer { url, token }
            if url == YOUTUBE_URL && token == "access-token"
    )));
}

#[test]
fn business_request_uses_connector_base_and_json_bearer_auth() {
    let fixture = fixture();
    fixture.runner.on(
        &["auth", "status"],
        FakeReply::ok(
            r#"{"user":"person@example.com","scopes":["https://www.googleapis.com/auth/youtube.readonly"],"token_valid":true}"#,
        ),
    );
    fixture.runner.on(
        &["auth", "export", "--unmasked"],
        FakeReply::ok(r#"{"refresh_token":"refresh-token"}"#),
    );
    fixture.http.on(
        TOKEN_URL,
        HttpResponse::ok(r#"{"access_token":"access-token","expires_in":3600}"#),
    );
    fixture.http.on(
        "https://www.googleapis.com/youtube/v3/channels",
        HttpResponse::ok(r#"{"items":[]}"#),
    );

    let response = fixture
        .core
        .business_request(BusinessConnector::Youtube, "get", "/channels", None)
        .unwrap();

    assert_eq!(response["items"], serde_json::json!([]));
    assert!(fixture.http.requests().iter().any(|request| matches!(
        request,
        HttpRequest::RequestBearer { method, url, token, body, .. }
            if method == "GET"
                && url == "https://www.googleapis.com/youtube/v3/channels"
                && token == "access-token"
                && body.is_none()
    )));
}

#[test]
fn business_request_rejects_write_without_write_access() {
    let fixture = fixture();
    fixture.runner.on(
        &["auth", "status"],
        FakeReply::ok(
            r#"{"scopes":["https://www.googleapis.com/auth/youtube.readonly"],"token_valid":true}"#,
        ),
    );

    let error = fixture
        .core
        .business_request(BusinessConnector::Youtube, "POST", "/channels", None)
        .unwrap_err();

    assert!(matches!(error, CoreError::Gws { exit_code: 403, .. }));
    assert!(fixture.http.requests().is_empty());
}

#[test]
fn business_request_rejects_unsafe_paths() {
    let fixture = fixture();

    let error = fixture
        .core
        .business_request(
            BusinessConnector::Youtube,
            "GET",
            "/channels/../videos",
            None,
        )
        .unwrap_err();

    assert!(matches!(error, CoreError::Parse(message) if message.contains("safe")));
}

#[test]
fn profile_picture_reads_google_userinfo_with_bearer_auth() {
    let fixture = fixture();
    fixture.runner.on(
        &["auth", "export", "--unmasked"],
        FakeReply::ok(r#"{"refresh_token":"refresh-token"}"#),
    );
    fixture.http.on(
        TOKEN_URL,
        HttpResponse::ok(r#"{"access_token":"access-token","expires_in":3600}"#),
    );
    fixture.http.on(
        "https://www.googleapis.com/oauth2/v3/userinfo",
        HttpResponse::ok(r#"{"picture":"https://lh3.googleusercontent.com/a/profile"}"#),
    );

    assert_eq!(
        fixture.core.profile_picture().unwrap().as_deref(),
        Some("https://lh3.googleusercontent.com/a/profile")
    );
}
