//! Shared-OAuth business connectors.
//!
//! The business connector catalogue lives in [`crate::scopes`]. This module owns
//! the active YouTube adapter; the remaining catalogue entries are planned
//! connectors and intentionally do not have implementations here yet.

use std::path::Path;

use serde_json::Value;

use crate::oauth_client::{CLIENT_ID, CLIENT_SECRET};
use crate::{BusinessConnector, Core, CoreError, Level};

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v3/userinfo";
const YOUTUBE_CHANNELS_URL: &str = "https://www.googleapis.com/youtube/v3/channels?part=snippet,contentDetails,statistics&mine=true";

impl Core {
    /// Returns the authenticated user's YouTube channel resource.
    ///
    /// The refresh token is read through `gws auth export --unmasked`, exchanged
    /// for a short-lived access token, and used only in an Authorization header.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use haulmer_googleworkspace_mcp_core::{Core, Paths};
    /// # let core = Core::system(Paths { home: "/h".into(), app_data: "/a".into() });
    /// let channel = core.youtube_channel()?;
    /// println!("{}", channel["items"][0]["snippet"]["title"]);
    /// # Ok::<(), haulmer_googleworkspace_mcp_core::CoreError>(())
    /// ```
    pub fn youtube_channel(&self) -> Result<Value, CoreError> {
        YoutubeConnector { core: self }.fetch()
    }

    /// Returns the authenticated user's Google profile picture URL, when available.
    pub fn profile_picture(&self) -> Result<Option<String>, CoreError> {
        let Some(gws) = self.find_gws()? else {
            return Ok(None);
        };
        let connector = YoutubeConnector { core: self };
        let refresh_token = connector.refresh_token(&gws)?;
        let access_token = connector.exchange_refresh_token(&refresh_token)?;
        let response = self.http.get_bearer(USERINFO_URL, &access_token)?;
        if response.status != 200 {
            return Err(CoreError::Http(format!(
                "Google UserInfo answered HTTP {}: {}",
                response.status,
                String::from_utf8_lossy(&response.body)
            )));
        }
        let profile: Value = serde_json::from_slice(&response.body)?;
        Ok(profile["picture"].as_str().map(str::to_string))
    }

    /// Executes an allowlisted Google business API request with the shared OAuth account.
    ///
    /// `path` is relative to the connector's documented API base and `body` is sent as JSON.
    /// GET requests require Read access; every other method requires Write access.
    pub fn business_request(
        &self,
        connector: BusinessConnector,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value, CoreError> {
        let method = method.to_ascii_uppercase();
        let base = business_base(connector, path)?;
        let level = if method == "GET" {
            Level::Read
        } else {
            Level::Write
        };
        let status = self.status()?;
        if !status.business_granted.contains(connector, level) {
            return Err(CoreError::Gws {
                exit_code: 403,
                message: format!("{connector:?} requires {level:?} access"),
            });
        }
        let gws = self.find_gws()?.ok_or_else(|| CoreError::Gws {
            exit_code: 2,
            message: "gws is not installed; open fraccional-googleworkspace-mcp and click Save"
                .into(),
        })?;
        let refresh_token = YoutubeConnector { core: self }.refresh_token(&gws)?;
        let access_token =
            YoutubeConnector { core: self }.exchange_refresh_token(&refresh_token)?;
        let developer_token = if connector == BusinessConnector::GoogleAds {
            Some(
                std::env::var("GOOGLE_ADS_DEVELOPER_TOKEN")
                    .map_err(|_| CoreError::NotImplemented("GOOGLE_ADS_DEVELOPER_TOKEN"))?,
            )
        } else {
            None
        };
        let headers = developer_token
            .as_deref()
            .map(|token| vec![("developer-token", token)])
            .unwrap_or_default();
        let json = body.map(Value::to_string);
        let response = self.http.request_bearer(
            &method,
            &base,
            &access_token,
            json.as_deref().map(str::as_bytes),
            &headers,
        )?;
        if !(200..300).contains(&response.status) {
            return Err(CoreError::Http(format!(
                "{connector:?} answered HTTP {}: {}",
                response.status,
                String::from_utf8_lossy(&response.body)
            )));
        }
        serde_json::from_slice(&response.body).map_err(Into::into)
    }
}

fn business_base(connector: BusinessConnector, path: &str) -> Result<String, CoreError> {
    if !path.starts_with('/') || path.contains("..") || path.contains('\0') {
        return Err(CoreError::Parse(
            "business API path must be relative and safe".into(),
        ));
    }
    let base = match connector {
        BusinessConnector::Youtube => "https://www.googleapis.com/youtube/v3",
        BusinessConnector::GoogleAds => "https://googleads.googleapis.com/v20",
        BusinessConnector::Analytics => "https://analyticsadmin.googleapis.com/v1beta",
        BusinessConnector::SearchConsole => "https://searchconsole.googleapis.com/webmasters/v3",
        BusinessConnector::BusinessProfile if path.starts_with("/accounts") => {
            "https://mybusinessaccountmanagement.googleapis.com/v1"
        }
        BusinessConnector::BusinessProfile => {
            "https://mybusinessbusinessinformation.googleapis.com/v1"
        }
        BusinessConnector::MerchantCenter => "https://shoppingcontent.googleapis.com/content/v2.1",
    };
    Ok(format!("{base}{path}"))
}

struct YoutubeConnector<'a> {
    core: &'a Core,
}

impl YoutubeConnector<'_> {
    fn fetch(&self) -> Result<Value, CoreError> {
        let gws = self.core.find_gws()?.ok_or_else(|| CoreError::Gws {
            exit_code: 2,
            message: "gws is not installed; open fraccional-googleworkspace-mcp and click Save"
                .into(),
        })?;
        let refresh_token = self.refresh_token(&gws)?;
        let access_token = self.exchange_refresh_token(&refresh_token)?;
        let response = self
            .core
            .http
            .get_bearer(YOUTUBE_CHANNELS_URL, &access_token)?;
        if response.status != 200 {
            return Err(CoreError::Http(format!(
                "YouTube answered HTTP {}: {}",
                response.status,
                String::from_utf8_lossy(&response.body)
            )));
        }
        Ok(serde_json::from_slice(&response.body)?)
    }

    fn refresh_token(&self, gws: &Path) -> Result<String, CoreError> {
        let output = self.core.runner.run(
            &self
                .core
                .gws_command(gws, &["auth", "export", "--unmasked"])?,
        )?;
        let json: Value = serde_json::from_str(&output.stdout)?;
        json["refresh_token"]
            .as_str()
            .filter(|_| output.exit_code == 0)
            .map(str::to_owned)
            .ok_or_else(|| CoreError::Gws {
                exit_code: output.exit_code,
                message: "could not read the refresh token".into(),
            })
    }

    fn exchange_refresh_token(&self, refresh_token: &str) -> Result<String, CoreError> {
        let response = self.core.http.post_form(
            TOKEN_URL,
            &[
                ("client_id", CLIENT_ID),
                ("client_secret", CLIENT_SECRET),
                ("refresh_token", refresh_token),
                ("grant_type", "refresh_token"),
            ],
        )?;
        if response.status != 200 {
            return Err(CoreError::Http(format!(
                "Google token endpoint answered HTTP {}",
                response.status
            )));
        }
        let json: Value = serde_json::from_slice(&response.body)?;
        json["access_token"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| CoreError::Parse("Google token response has no access_token".into()))
    }
}
