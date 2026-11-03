//! the innertube calls behind music.youtube.com: one post per request, with
//! the browser's cookie and the account's `SAPISIDHASH`.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use reqwest::header::{CONTENT_TYPE, COOKIE, ORIGIN, REFERER, USER_AGENT};
use serde_json::{Value, json};

use super::auth::{Auth, USER_AGENT_VALUE};

const API_ROOT: &str = "https://music.youtube.com/youtubei/v1/";
const ORIGIN_URL: &str = "https://music.youtube.com";
const CLIENT_VERSION: &str = "1.20241127.01.00";

pub const SEARCH_SONGS: &str = "EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D";
pub const SEARCH_ALBUMS: &str = "EgWKAQIYAWoKEAkQChAFEAMQBA%3D%3D";
pub const SEARCH_ARTISTS: &str = "EgWKAQIgAWoKEAkQChAFEAMQBA%3D%3D";
pub const SEARCH_PLAYLISTS: &str = "EgWKAQIoAWoKEAkQChAFEAMQBA%3D%3D";

#[derive(Clone)]
pub struct Innertube {
    http: reqwest::Client,
    auth: Option<Auth>,
    /// drops sponsored shelves and every ad placement before anything reads
    /// the answer
    network_filter: Arc<AtomicBool>,
}

impl Innertube {
    pub fn new(auth: Option<Auth>, network_filter: bool) -> Result<Self, String> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(40))
            .user_agent(USER_AGENT_VALUE)
            .build()
            .map_err(|error| format!("could not start the YouTube Music client: {error}"))?;
        Ok(Self {
            http,
            auth,
            network_filter: Arc::new(AtomicBool::new(network_filter)),
        })
    }

    pub fn auth(&self) -> Option<&Auth> {
        self.auth.as_ref()
    }

    pub fn set_auth(&mut self, auth: Option<Auth>) {
        self.auth = auth;
    }

    pub fn set_network_filter(&self, enabled: bool) {
        self.network_filter.store(enabled, Ordering::Relaxed);
    }

    pub fn network_filter(&self) -> bool {
        self.network_filter.load(Ordering::Relaxed)
    }

    pub async fn send(&self, endpoint: &str, body: Value) -> Result<Value, String> {
        let mut body = body;
        if let Value::Object(map) = &mut body {
            map.insert("context".to_owned(), context());
        }

        let url = format!("{API_ROOT}{endpoint}?prettyPrint=false");
        let mut request = self
            .http
            .post(url)
            .header(CONTENT_TYPE, "application/json")
            .header(ORIGIN, ORIGIN_URL)
            .header(REFERER, format!("{ORIGIN_URL}/"))
            .header(USER_AGENT, USER_AGENT_VALUE)
            .header("X-Goog-AuthUser", "0")
            .header("X-Origin", ORIGIN_URL);

        if let Some(auth) = &self.auth {
            request = request
                .header("Authorization", auth.authorization_header())
                .header(COOKIE, &auth.cookie);
        }

        let response = request
            .body(body.to_string())
            .send()
            .await
            .map_err(|error| format!("could not reach YouTube Music: {error}"))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|error| format!("could not read the YouTube Music answer: {error}"))?;

        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(
                "YouTube Music refused the request. The saved login has probably expired; sign in again."
                    .to_owned(),
            );
        }
        if status.as_u16() == 429 {
            return Err("YouTube Music is rate limiting this account. Try again shortly.".to_owned());
        }
        if !status.is_success() {
            let snippet: String = text.chars().take(240).collect();
            return Err(format!(
                "YouTube Music returned HTTP {}: {}",
                status.as_u16(),
                snippet.trim()
            ));
        }

        let mut root: Value = serde_json::from_str(&text)
            .map_err(|error| format!("YouTube Music sent something that is not JSON: {error}"))?;
        if let Some(error) = root.get("error").and_then(Value::as_object) {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("the request was refused");
            return Err(format!("YouTube Music: {message}"));
        }
        if self.network_filter() {
            scrub_sponsored(&mut root);
        }
        Ok(root)
    }

    pub async fn browse(
        &self,
        browse_id: &str,
        params: Option<&str>,
        continuation: Option<&str>,
    ) -> Result<Value, String> {
        let mut body = json!({ "browseId": browse_id });
        if let Value::Object(map) = &mut body {
            if let Some(params) = params {
                map.insert("params".to_owned(), Value::String(params.to_owned()));
            }
            if let Some(continuation) = continuation {
                map.insert(
                    "continuation".to_owned(),
                    Value::String(continuation.to_owned()),
                );
            }
        }
        self.send("browse", body).await
    }

    pub async fn search(&self, query: &str, params: Option<&str>) -> Result<Value, String> {
        let mut body = json!({ "query": query });
        if let (Value::Object(map), Some(params)) = (&mut body, params) {
            map.insert("params".to_owned(), Value::String(params.to_owned()));
        }
        self.send("search", body).await
    }

    pub async fn next(&self, video_id: &str) -> Result<Value, String> {
        self.send("next", json!({ "videoId": video_id })).await
    }

    pub async fn mutate(&self, endpoint: &str, body: Value) -> Result<Value, String> {
        self.send(endpoint, body).await
    }
}

fn context() -> Value {
    json!({
        "client": {
            "clientName": "WEB_REMIX",
            "clientVersion": CLIENT_VERSION,
            "hl": "en",
            "gl": "US",
            "userAgent": USER_AGENT_VALUE,
            "platform": "DESKTOP",
        },
        "user": { "lockedSafetyMode": false },
        "request": { "useSsl": true },
    })
}

/// removes what a filtered resolver would: promoted shelves, ad slots, and the
/// player's ad breaks. only the client's own answers are touched; the audio
/// itself is fetched through yt-dlp, which never puts an ad in it.
pub fn scrub_sponsored(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|key, _| {
                !matches!(
                    key.as_str(),
                    "adPlacements"
                        | "playerAds"
                        | "adSlots"
                        | "adBreakHeartbeatParams"
                        | "adParams"
                        | "adBreakServiceRenderer"
                ) && !key.contains("Promoted")
                    && !key.contains("AdSlot")
            });
            for child in map.values_mut() {
                scrub_sponsored(child);
            }
        }
        Value::Array(items) => {
            items.retain(|item| {
                if let Value::Object(map) = item {
                    !map.contains_key("musicPromotedItemRenderer")
                        && !map.contains_key("adSlotRenderer")
                        && !map.contains_key("promotedSparklesWebRenderer")
                } else {
                    true
                }
            });
            for child in items.iter_mut() {
                scrub_sponsored(child);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_network_filtered_answer_keeps_the_shelves() {
        let mut value = json!({
            "contents": {
                "shelf": { "title": "Listen again", "items": [{"musicResponsiveListItemRenderer": {}}] },
                "adSlots": [{ "adSlotRenderer": {} }],
                "promoted": [{ "musicPromotedItemRenderer": {} }]
            },
            "playerAds": [{ "playerLegacyDesktopWatchAdsRenderer": {} }]
        });
        scrub_sponsored(&mut value);
        assert!(value["contents"]["shelf"]["items"].is_array());
        assert!(value["contents"].get("adSlots").is_none());
        assert!(
            value["contents"]["promoted"]
                .as_array()
                .is_none_or(|items| items.is_empty())
        );
        assert!(value.get("playerAds").is_none());
    }
}
