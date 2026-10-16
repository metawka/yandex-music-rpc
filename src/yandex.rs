use std::collections::HashMap;
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde_json::Value;
use windows::Foundation::{AsyncStatus, Uri};
use windows::Web::Http::HttpClient;
use windows::core::HSTRING;

const API: &str = "https://api.music.yandex.net/search?type=track&page=0&text=";
const SITE: &str = "https://music.yandex.ru";

#[derive(Clone, Debug, Default)]
pub struct Meta {
    pub track_url: String,
    pub artist_url: Option<String>,
    pub album_url: Option<String>,
    pub album_title: Option<String>,
    pub cover: Option<String>,
}

pub struct Yandex {
    client: Option<HttpClient>,
    cache: HashMap<String, Meta>,
}

impl Yandex {
    pub fn new() -> Self {
        let client = HttpClient::new().ok();
        if let Some(client) = &client {
            if let Ok(headers) = client.DefaultRequestHeaders() {
                let _ = headers.UserAgent().and_then(|ua| ua.TryParseAdd(&HSTRING::from("Mozilla/5.0 yandex-music-rpc")));
            }
        }
        Self { client, cache: HashMap::new() }
    }

    pub fn find(&mut self, title: &str, artist: &str, duration_ms: i64) -> Meta {
        let key = format!("{title}\u{1}{artist}");
        if let Some(meta) = self.cache.get(&key) {
            return meta.clone();
        }
        let query = format!("{artist} {title}");
        let fallback = Meta { track_url: format!("{SITE}/search?text={}", encode(&query)), ..Default::default() };
        let Some(body) = self.fetch(&query) else {
            crate::config::log(&format!("search failed: {query}"));
            return fallback;
        };
        let meta = pick(&body, title, artist, duration_ms).unwrap_or(fallback);
        if self.cache.len() > 500 {
            self.cache.clear();
        }
        self.cache.insert(key, meta.clone());
        meta
    }

    fn fetch(&self, query: &str) -> Option<Value> {
        let client = self.client.as_ref()?;
        let uri = Uri::CreateUri(&HSTRING::from(format!("{API}{}", encode(query)))).ok()?;
        let op = client.GetStringAsync(&uri).ok()?;
        let deadline = Instant::now() + Duration::from_secs(10);
        while op.Status().ok()? == AsyncStatus::Started {
            if Instant::now() > deadline {
                let _ = op.Cancel();
                return None;
            }
            sleep(Duration::from_millis(50));
        }
        let body = op.GetResults().ok()?.to_string();
        serde_json::from_str(&body).ok()
    }
}

fn pick(body: &Value, title: &str, artist: &str, duration_ms: i64) -> Option<Meta> {
    let results = body["result"]["tracks"]["results"].as_array()?;
    let want_title = norm(title);
    let want_artist = norm(artist);

    let best = results
        .iter()
        .map(|t| {
            let mut score = 0;
            let got_title = norm(t["title"].as_str().unwrap_or_default());
            if got_title == want_title {
                score += 3;
            } else if got_title.contains(&want_title) || want_title.contains(&got_title) {
                score += 1;
            }
            let artists = t["artists"].as_array().cloned().unwrap_or_default();
            if artists.iter().any(|a| {
                let name = norm(a["name"].as_str().unwrap_or_default());
                !name.is_empty() && want_artist.contains(&name)
            }) {
                score += 2;
            }
            let got_duration = t["durationMs"].as_i64().unwrap_or(0);
            if duration_ms > 0 && (got_duration - duration_ms).abs() < 3000 {
                score += 2;
            }
            (score, t)
        })
        .filter(|(score, _)| *score >= 4)
        .max_by_key(|(score, _)| *score)?
        .1;

    let id = id_of(&best["id"])?;
    let album = best["albums"].as_array().and_then(|a| a.first());
    let album_id = album.and_then(|a| id_of(&a["id"]));
    let artist_id = best["artists"].as_array().and_then(|a| a.first()).and_then(|a| id_of(&a["id"]));
    let cover = best["coverUri"]
        .as_str()
        .or_else(|| album.and_then(|a| a["coverUri"].as_str()))
        .map(|uri| format!("https://{}", uri.replace("%%", "400x400")));

    Some(Meta {
        track_url: match &album_id {
            Some(album_id) => format!("{SITE}/album/{album_id}/track/{id}"),
            None => format!("{SITE}/track/{id}"),
        },
        artist_url: artist_id.map(|a| format!("{SITE}/artist/{a}")),
        album_url: album_id.map(|a| format!("{SITE}/album/{a}")),
        album_title: album.and_then(|a| a["title"].as_str()).map(str::to_string),
        cover,
    })
}

fn id_of(v: &Value) -> Option<String> {
    match v {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

fn norm(s: &str) -> String {
    s.to_lowercase().replace('ё', "е").trim().to_string()
}

pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
