use std::time::{SystemTime, UNIX_EPOCH};

use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::core::Result;

const FILETIME_UNIX_DIFF: i64 = 116_444_736_000_000_000;

#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playing: bool,
    pub elapsed_ms: i64,
    pub duration_ms: i64,
}

pub struct Smtc {
    manager: Manager,
}

impl Smtc {
    pub fn new() -> Result<Self> {
        Ok(Self { manager: Manager::RequestAsync()?.get()? })
    }

    pub fn sources(&self) -> Vec<String> {
        let Ok(sessions) = self.manager.GetSessions() else { return Vec::new() };
        sessions
            .into_iter()
            .filter_map(|s| s.SourceAppUserModelId().ok().map(|id| id.to_string()))
            .collect()
    }

    pub fn current(&self) -> Option<Track> {
        let sessions = self.manager.GetSessions().ok()?;
        let session = sessions.into_iter().find(|s| {
            s.SourceAppUserModelId().map(|id| is_yandex(&id.to_string())).unwrap_or(false)
        })?;
        read(&session).ok().flatten()
    }
}

pub fn is_yandex(id: &str) -> bool {
    let id = id.to_lowercase();
    id.contains("yandex") || id.contains("яндекс")
}

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn read(session: &Session) -> Result<Option<Track>> {
    let props = session.TryGetMediaPropertiesAsync()?.get()?;
    let title = props.Title()?.to_string().trim().to_string();
    if title.is_empty() {
        return Ok(None);
    }
    let artist = props.Artist()?.to_string().trim().to_string();
    let album = props.AlbumTitle()?.to_string().trim().to_string();
    let playing = session.GetPlaybackInfo()?.PlaybackStatus()? == Status::Playing;

    let timeline = session.GetTimelineProperties()?;
    let start = timeline.StartTime()?.Duration;
    let end = timeline.EndTime()?.Duration;
    let position = timeline.Position()?.Duration;
    let updated = (timeline.LastUpdatedTime()?.UniversalTime - FILETIME_UNIX_DIFF) / 10_000;

    let duration_ms = ((end - start) / 10_000).max(0);
    let mut elapsed_ms = ((position - start) / 10_000).max(0);
    if playing && updated > 0 {
        elapsed_ms += (now_ms() - updated).max(0);
    }
    if duration_ms > 0 {
        elapsed_ms = elapsed_ms.min(duration_ms);
    }

    Ok(Some(Track { title, artist, album, playing, elapsed_ms, duration_ms }))
}
