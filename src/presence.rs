use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

use serde_json::{Value, json};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};

use crate::config;
use crate::discord::Discord;
use crate::smtc::{Smtc, Track, now_ms};
use crate::yandex::{Meta, Yandex};

pub const CLIENT_ID: &str = "1560696687004225686";
pub const REPO: &str = "https://github.com/metawka/yandex-music-rpc";
const LOGO: &str = "https://raw.githubusercontent.com/metawka/yandex-music-rpc/main/assets/logo.png";

pub struct Shared {
    pub now_playing: Mutex<String>,
    pub discord: AtomicBool,
}

struct Sent {
    title: String,
    artist: String,
    start: i64,
    at: i64,
}

pub fn run(shared: Arc<Shared>) {
    unsafe {
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
    }
    let mut smtc: Option<Smtc> = None;
    let mut yandex = Yandex::new();
    let mut discord = Discord::new(CLIENT_ID);
    let mut last: Option<Sent> = None;
    let mut retry_at = 0;
    let mut sources: Vec<String> = Vec::new();
    let mut reply_logged = false;

    loop {
        if smtc.is_none() {
            smtc = Smtc::new().map_err(|e| config::log(&format!("smtc: {e}"))).ok();
        }
        if let Some(s) = &smtc {
            let seen = s.sources();
            if seen != sources {
                config::log(&format!("media sources: {seen:?}"));
                sources = seen;
            }
        }

        let track = smtc.as_ref().and_then(|s| s.current());
        *shared.now_playing.lock().unwrap() = match &track {
            Some(t) if t.artist.is_empty() => t.title.clone(),
            Some(t) => format!("{} - {}", t.artist, t.title),
            None => String::new(),
        };

        let now = now_ms();
        match track.filter(|t| t.playing) {
            Some(track) => {
                if !discord.connected() && now >= retry_at {
                    match discord.connect() {
                        Ok(()) => {
                            config::log("discord connected");
                            last = None;
                            reply_logged = false;
                        }
                        Err(e) => {
                            config::log(&format!("discord: {e}"));
                            retry_at = now + 5000;
                        }
                    }
                }
                if discord.connected() {
                    let start = now - track.elapsed_ms;
                    if needs_update(&last, &track, start, now) {
                        let meta = yandex.find(&track.title, &track.artist, track.duration_ms);
                        match discord.set_activity(Some(&build(&track, &meta, start))) {
                            Ok(reply) => {
                                if reply["evt"] == "ERROR" || !reply_logged {
                                    config::log(&format!("discord reply: {reply}"));
                                    reply_logged = true;
                                }
                                last = Some(Sent {
                                    title: track.title.clone(),
                                    artist: track.artist.clone(),
                                    start,
                                    at: now,
                                });
                            }
                            Err(e) => {
                                config::log(&format!("discord write: {e}"));
                                last = None;
                            }
                        }
                    }
                }
            }
            None => {
                if last.is_some() && discord.connected() {
                    let _ = discord.set_activity(None);
                }
                last = None;
            }
        }

        shared.discord.store(discord.connected(), Ordering::Relaxed);
        sleep(Duration::from_secs(1));
    }
}

pub fn dump() -> String {
    unsafe {
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
    }
    let Ok(smtc) = Smtc::new() else { return "smtc unavailable".into() };
    let mut out = format!("sources: {:?}\n", smtc.sources());
    match smtc.current() {
        Some(track) => {
            let meta = Yandex::new().find(&track.title, &track.artist, track.duration_ms);
            let activity = build(&track, &meta, now_ms() - track.elapsed_ms);
            out += &format!("track: {track:?}\n");
            out += &format!("activity: {}\n", serde_json::to_string_pretty(&activity).unwrap_or_default());
        }
        None => out += "track: none\n",
    }
    out
}

fn needs_update(last: &Option<Sent>, track: &Track, start: i64, now: i64) -> bool {
    let Some(last) = last else { return true };
    last.title != track.title
        || last.artist != track.artist
        || (last.start - start).abs() > 2500
        || now - last.at > 60_000
}

fn build(track: &Track, meta: &Meta, start: i64) -> Value {
    let mut activity = json!({
        "type": 2,
        "status_display_type": 1,
        "details": fit(&track.title),
        "details_url": meta.track_url,
    });

    if !track.artist.is_empty() {
        activity["state"] = json!(fit(&track.artist));
        if let Some(url) = &meta.artist_url {
            activity["state_url"] = json!(url);
        }
    }

    let mut assets = json!({ "large_image": meta.cover.clone().unwrap_or_else(|| LOGO.to_string()) });
    let album = if track.album.is_empty() { meta.album_title.clone().unwrap_or_default() } else { track.album.clone() };
    if album.chars().count() >= 2 && !same(&album, &track.title) {
        assets["large_text"] = json!(fit(&album));
    }
    if let Some(url) = &meta.album_url {
        assets["large_url"] = json!(url);
    }
    activity["assets"] = assets;

    activity["timestamps"] = if track.duration_ms > 0 {
        json!({ "start": start, "end": start + track.duration_ms })
    } else {
        json!({ "start": start })
    };

    activity["buttons"] = json!([
        { "label": "Слушать трек", "url": meta.track_url },
        { "label": "RPC на GitHub", "url": REPO },
    ]);
    activity
}

fn same(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

fn fit(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if out.len() + c.len_utf8() > 128 {
            break;
        }
        out.push(c);
    }
    while out.chars().count() < 2 {
        out.push('\u{2800}');
    }
    out
}
