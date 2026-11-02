//! signing in the way music.youtube.com does: the browser's cookie, and the
//! `SAPISIDHASH` header every authenticated request carries.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::header::{CONTENT_TYPE, COOKIE, ORIGIN, REFERER, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha1::{Digest, Sha1};

use super::models::{Profile, Thumbnail};

const ACCOUNT_MENU_URL: &str =
    "https://music.youtube.com/youtubei/v1/account/account_menu?prettyPrint=false";
const ORIGIN_URL: &str = "https://music.youtube.com";
const SAPISID_KEYS: [&str; 3] = ["SAPISID", "__Secure-3PAPISID", "__Secure-1PASID"];

pub const USER_AGENT_VALUE: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Auth {
    pub cookie: String,
    pub sapisid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub handle: String,
    #[serde(default)]
    pub photo: String,
}

impl Auth {
    pub fn from_json(raw: &str) -> Result<Self, String> {
        serde_json::from_str(raw).map_err(|error| format!("the saved login is damaged: {error}"))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn authorization_header(&self) -> String {
        let timestamp = unix_timestamp().unwrap_or_default();
        sapisid_hash(&self.sapisid, timestamp)
    }

    pub fn profile(&self) -> Profile {
        Profile {
            id: if self.handle.is_empty() {
                self.name.clone()
            } else {
                self.handle.clone()
            },
            name: self.name.clone(),
            handle: self.handle.clone(),
            thumbnails: if self.photo.is_empty() {
                Vec::new()
            } else {
                vec![Thumbnail::new(self.photo.clone(), 512, 512)]
            },
        }
    }
}

/// checks a cookie with youtube music and takes the account's name from it.
pub async fn sign_in(cookie: &str) -> Result<Auth, String> {
    let cookie = strip_cookie_prefix(cookie.trim()).trim().to_owned();
    if cookie.is_empty() {
        return Err("Paste the Cookie header from a signed-in music.youtube.com.".to_owned());
    }
    let sapisid = find_sapisid(&cookie).ok_or_else(|| {
        "That cookie has no SAPISID, __Secure-3PAPISID or __Secure-1PASID. Copy the whole Cookie header from a signed-in music.youtube.com.".to_owned()
    })?;

    let mut auth = Auth {
        cookie,
        sapisid,
        ..Auth::default()
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .user_agent(USER_AGENT_VALUE)
        .build()
        .map_err(|error| format!("could not start the login request: {error}"))?;

    let body = json!({
        "context": { "client": { "clientName": "WEB_REMIX", "clientVersion": "1.20241127.01.00", "hl": "en", "gl": "US" } },
    });

    let response = client
        .post(ACCOUNT_MENU_URL)
        .header(CONTENT_TYPE, "application/json")
        .header(ORIGIN, ORIGIN_URL)
        .header(REFERER, format!("{ORIGIN_URL}/"))
        .header(USER_AGENT, USER_AGENT_VALUE)
        .header("X-Goog-AuthUser", "0")
        .header("Authorization", auth.authorization_header())
        .header(COOKIE, &auth.cookie)
        .body(body.to_string())
        .send()
        .await
        .map_err(|error| format!("could not reach YouTube Music: {error}"))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| format!("could not read the YouTube Music answer: {error}"))?;
    if !status.is_success() {
        let snippet: String = text.chars().take(240).collect();
        return Err(format!(
            "YouTube Music rejected the cookie (HTTP {}). Copy a fresh Cookie header from a signed-in music.youtube.com. {}",
            status.as_u16(),
            snippet.trim()
        ));
    }

    let root: Value = serde_json::from_str(&text)
        .map_err(|error| format!("YouTube Music sent something that is not JSON: {error}"))?;
    let name = find_string(&root, "accountName")
        .or_else(|| find_string(&root, "displayName"))
        .ok_or_else(|| {
            "YouTube Music accepted the request but returned no account. The cookie is probably expired or missing SAPISID.".to_owned()
        })?;
    auth.name = name;
    auth.handle = find_string(&root, "channelHandle").unwrap_or_default();
    auth.photo = find_string(&root, "accountPhotoUrl")
        .or_else(|| find_string(&root, "avatarUrl"))
        .unwrap_or_default();
    Ok(auth)
}

fn strip_cookie_prefix(raw: &str) -> &str {
    match raw.get(..7) {
        Some(prefix) if prefix.eq_ignore_ascii_case("cookie:") => &raw[7..],
        _ => raw,
    }
}

fn find_sapisid(cookie: &str) -> Option<String> {
    let pairs: Vec<(&str, &str)> = cookie
        .split(';')
        .filter_map(|part| part.trim().split_once('='))
        .map(|(key, value)| (key.trim(), value.trim()))
        .collect();
    for wanted in SAPISID_KEYS {
        if let Some((_, value)) = pairs.iter().find(|(key, _)| *key == wanted)
            && !value.is_empty()
        {
            return Some((*value).to_owned());
        }
    }
    None
}

fn unix_timestamp() -> Result<u64, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "the system clock is before the unix epoch".to_owned())?
        .as_secs())
}

fn sapisid_hash(sapisid: &str, timestamp: u64) -> String {
    let digest = Sha1::digest(format!("{timestamp} {sapisid} {ORIGIN_URL}").as_bytes());
    let mut hex = String::with_capacity(40);
    for byte in digest.as_slice() {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    format!("SAPISIDHASH {timestamp}_{hex}")
}

fn find_string(value: &Value, key: &str) -> Option<String> {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(text)) = map.get(key)
                && !text.is_empty()
            {
                return Some(text.clone());
            }
            if let Some(Value::Object(runs)) = map.get(key)
                && let Some(text) = runs_text(&Value::Object(runs.clone()))
            {
                return Some(text);
            }
            map.values().find_map(|child| find_string(child, key))
        }
        Value::Array(items) => items.iter().find_map(|child| find_string(child, key)),
        _ => None,
    }
}

fn runs_text(value: &Value) -> Option<String> {
    let runs = value.get("runs")?.as_array()?;
    let text: String = runs
        .iter()
        .filter_map(|run| run.get("text").and_then(Value::as_str))
        .collect();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_cookie_the_browser_copied() {
        let cookie = "Cookie: PREF=hl=en; SAPISID=abc123; __Secure-3PSID=def";
        let sapisid = find_sapisid(strip_cookie_prefix(cookie)).unwrap();
        assert_eq!(sapisid, "abc123");
    }

    #[test]
    fn hashes_as_youtube_expects() {
        let header = sapisid_hash("abc123", 1_700_000_000);
        assert!(header.starts_with("SAPISIDHASH 1700000000_"));
        assert_eq!(header.len(), "SAPISIDHASH ".len() + 10 + 1 + 40);
    }

    #[test]
    fn the_secure_variant_is_a_fallback() {
        let cookie = "PREF=hl=en; __Secure-3PAPISID=xyz";
        assert_eq!(find_sapisid(cookie).unwrap(), "xyz");
    }
}
