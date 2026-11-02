//! synchronized lyrics from lrclib, and plain lyrics as a fallback.
//!
//! youtube music's own lyrics page is usually plain text. lrclib often has the
//! same song timed line by line, which is what the immersive player wants, so
//! it is asked first and youtube music's page is the fallback.

use std::time::Duration;

use reqwest::Url;
use serde::Deserialize;
use serde_json::{Value, json};

const API_GET: &str = "https://lrclib.net/api/get";
const API_SEARCH: &str = "https://lrclib.net/api/search";
const USER_AGENT: &str = "Tubify/0.1 (https://github.com/swiftify-org/swiftify)";

#[derive(Debug, Deserialize)]
struct Record {
    #[serde(default, rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
    #[serde(default, rename = "plainLyrics")]
    plain_lyrics: Option<String>,
}

pub async fn lrclib(artist: &str, title: &str, duration: Option<u32>) -> Option<Value> {
    let client = client()?;
    for (artist, title) in variants(artist, title) {
        if let Some(record) = lookup(&client, &artist, &title, duration).await
            && let Some(lyrics) = record.into_synced()
        {
            return Some(lyrics);
        }
    }
    for (artist, title) in variants(artist, title) {
        if let Some(lyrics) = search(&client, &artist, &title).await {
            return Some(lyrics);
        }
    }
    None
}

pub async fn lrclib_plain(artist: &str, title: &str, duration: Option<u32>) -> Option<Value> {
    let client = client()?;
    for (artist, title) in variants(artist, title) {
        if let Some(record) = lookup(&client, &artist, &title, duration).await
            && let Some(lyrics) = record.into_plain()
        {
            return Some(lyrics);
        }
    }
    None
}

fn client() -> Option<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent(USER_AGENT)
        .build()
        .ok()
}

impl Record {
    fn into_synced(self) -> Option<Value> {
        let text = self.synced_lyrics.filter(|text| !text.trim().is_empty())?;
        let lines = parse_synced(&text);
        if lines.is_empty() {
            return None;
        }
        Some(json!({
            "language": "en",
            "provider": "LRCLIB",
            "syncType": "lineSynced",
            "lines": lines,
        }))
    }

    fn into_plain(self) -> Option<Value> {
        let text = self.plain_lyrics.filter(|text| !text.trim().is_empty())?;
        let lines: Vec<Value> = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| json!({ "startTimeMs": 0, "endTimeMs": 0, "words": line }))
            .collect();
        (!lines.is_empty()).then_some(json!({
            "language": "en",
            "provider": "LRCLIB",
            "syncType": "unsynchronized",
            "lines": lines,
        }))
    }
}

fn variants(artist: &str, title: &str) -> Vec<(String, String)> {
    let clean = clean_title(title);
    let mut out: Vec<(String, String)> = Vec::new();
    let mut push = |artist: String, title: String| {
        if !artist.is_empty() && !title.is_empty() && !out.iter().any(|(a, t)| *a == artist && *t == title) {
            out.push((artist, title));
        }
    };
    push(artist.to_owned(), title.to_owned());
    push(artist.to_owned(), clean.clone());
    for token in artist.split([',', '&']) {
        let token = token.split("feat.").next().unwrap_or(token).trim();
        push(token.to_owned(), clean.clone());
        push(token.to_owned(), title.to_owned());
    }
    out
}

fn clean_title(title: &str) -> String {
    let lower = title.to_ascii_lowercase();
    let mut cut = title.len();
    for marker in [" (", " [", " - ", " feat.", " ft."] {
        if let Some(index) = lower.find(marker) {
            cut = cut.min(index);
        }
    }
    title[..cut].trim().to_owned()
}

fn endpoint(base: &str, params: &[(&str, String)]) -> Option<Url> {
    let mut url = Url::parse(base).ok()?;
    url.query_pairs_mut()
        .extend_pairs(params.iter().map(|(key, value)| (*key, value.as_str())));
    Some(url)
}

async fn lookup(
    client: &reqwest::Client,
    artist: &str,
    title: &str,
    duration: Option<u32>,
) -> Option<Record> {
    let mut params = vec![
        ("artist_name", artist.to_owned()),
        ("track_name", title.to_owned()),
    ];
    if let Some(duration) = duration {
        params.push(("duration", duration.to_string()));
    }
    let response = client.get(endpoint(API_GET, &params)?).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.json::<Record>().await.ok()
}

async fn search(client: &reqwest::Client, artist: &str, title: &str) -> Option<Value> {
    let query = format!("{artist} {title}");
    let response = client
        .get(endpoint(API_SEARCH, &[("q", query)])?)
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let records = response.json::<Vec<Record>>().await.ok()?;
    records.into_iter().find_map(Record::into_synced)
}

fn parse_synced(text: &str) -> Vec<Value> {
    let mut timed: Vec<(u64, String)> = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim_start();
        let mut stamps = Vec::new();
        while let Some((millis, consumed)) = timestamp(rest) {
            stamps.push(millis);
            rest = rest[consumed..].trim_start();
        }
        if stamps.is_empty() {
            continue;
        }
        let words = rest.trim();
        if words.is_empty() {
            continue;
        }
        for millis in stamps {
            timed.push((millis, words.to_owned()));
        }
    }
    timed.sort_by_key(|(millis, _)| *millis);
    let mut lines = Vec::with_capacity(timed.len());
    for (index, (millis, words)) in timed.iter().enumerate() {
        let end = timed
            .get(index + 1)
            .map(|(next, _)| *next)
            .unwrap_or(millis + 5_000);
        lines.push(json!({
            "startTimeMs": millis,
            "endTimeMs": end,
            "words": words,
        }));
    }
    lines
}

fn timestamp(input: &str) -> Option<(u64, usize)> {
    let rest = input.strip_prefix('[')?;
    let (stamp, after) = rest.split_once(']')?;
    let millis = clock(stamp)?;
    Some((millis, input.len() - after.len()))
}

fn clock(stamp: &str) -> Option<u64> {
    let parts: Vec<&str> = stamp.split(':').collect();
    let (hours, minutes, seconds) = match parts.as_slice() {
        [minutes, seconds] => (0.0, number(minutes)?, number(seconds)?),
        [hours, minutes, seconds] => (number(hours)?, number(minutes)?, number(seconds)?),
        _ => return None,
    };
    Some(((hours * 3600.0 + minutes * 60.0 + seconds) * 1000.0).round() as u64)
}

fn number(field: &str) -> Option<f64> {
    let field = field.trim();
    if field.is_empty() || !field.bytes().all(|byte| byte.is_ascii_digit() || byte == b'.') {
        return None;
    }
    field.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lrc_timestamps_become_lines() {
        let lines = parse_synced("[00:12.50]first\n[00:15.00]second\n");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["startTimeMs"], 12_500);
        assert_eq!(lines[1]["endTimeMs"], 20_000);
    }

    #[test]
    fn titles_lose_their_remix_suffixes() {
        assert_eq!(clean_title("Blinding Lights (Remix)"), "Blinding Lights");
    }
}
