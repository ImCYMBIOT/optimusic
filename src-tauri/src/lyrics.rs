//! Synced lyrics from LRCLIB (https://lrclib.net), a free, open lyrics database.
//!
//! YouTube Music on desktop only shows plain lyrics, with no timings, so lines can't be
//! highlighted or clicked. When "Synced lyrics" is on, lyrics.js asks for the current song's
//! timed lyrics here. The request is made by the app, not the page, so the page can only ever
//! ask "lyrics for this title/artist/length" and the app only ever talks to lrclib.net.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const API: &str = "https://lrclib.net/api";
/// LRCLIB asks apps to identify themselves. This names the app only, never the user.
const USER_AGENT: &str = concat!("Optimusic/", env!("CARGO_PKG_VERSION"), " (https://github.com/ImCYMBIOT/optimusic)");
/// A song's lyrics are a few KB; anything far bigger isn't lyrics.
const MAX_RESPONSE: usize = 512 * 1024;
const MAX_LINES: usize = 2000;
const CACHE_SIZE: usize = 64;
/// How far LRCLIB's song length may be from YouTube's, in seconds.
const DURATION_SLACK: f64 = 3.0;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Line {
    /// Start time in seconds
    t: f64,
    text: String,
}

#[derive(Default)]
pub struct Lyrics {
    /// Found lyrics (or None for "LRCLIB has none") per song, so replays cost nothing
    cache: Mutex<HashMap<String, Option<Arc<Vec<Line>>>>>,
}

pub fn manage(app: &AppHandle) {
    app.manage(Lyrics::default());
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    synced_lyrics: Option<String>,
}

/// From lyrics.js (music.youtube.com): timed lyrics for the playing song, or null if LRCLIB has
/// none. Refuses unless the user turned Synced lyrics on.
#[tauri::command]
pub async fn synced_lyrics(
    app: AppHandle,
    title: String,
    artist: String,
    album: Option<String>,
    duration: f64,
) -> Result<Option<Arc<Vec<Line>>>, String> {
    if !crate::customize::synced_lyrics_on(&app) {
        return Err("synced lyrics are off".into());
    }
    let title = clip(&title);
    let artist = clip(&artist);
    let album = album.as_deref().map(clip).filter(|a| !a.is_empty());
    if title.is_empty() || !duration.is_finite() || duration <= 0.0 || duration > 6.0 * 3600.0 {
        return Ok(None);
    }
    let key = format!("{title}\u{1}{artist}\u{1}{}", duration.round());
    if let Some(hit) = app.state::<Lyrics>().cache.lock().unwrap().get(&key) {
        return Ok(hit.clone());
    }

    let found = lookup(&title, &artist, album.as_deref(), duration).await.map_err(|e| e.to_string())?;
    let found = found.map(Arc::new);
    let state = app.state::<Lyrics>();
    let mut cache = state.cache.lock().unwrap();
    if cache.len() >= CACHE_SIZE {
        cache.clear();
    }
    cache.insert(key, found.clone());
    Ok(found)
}

type Error = Box<dyn std::error::Error + Send + Sync>;

/// Exact match first (LRCLIB's own matching, which allows ±2 s), then a search by title and
/// artist, then a looser search with "(Official Video)"-style extras removed.
async fn lookup(title: &str, artist: &str, album: Option<&str>, duration: f64) -> Result<Option<Vec<Line>>, Error> {
    let client = client()?;

    let mut get = url::Url::parse(&format!("{API}/get"))?;
    {
        let mut q = get.query_pairs_mut();
        q.append_pair("track_name", title).append_pair("artist_name", artist);
        if let Some(album) = album {
            q.append_pair("album_name", album);
        }
        q.append_pair("duration", &duration.round().to_string());
    }
    if let Some(record) = fetch::<Record>(&client, get).await? {
        if let Some(lines) = record.synced_lyrics.as_deref().map(parse_lrc).filter(|l| !l.is_empty()) {
            return Ok(Some(lines));
        }
    }

    let mut search = url::Url::parse(&format!("{API}/search"))?;
    search.query_pairs_mut().append_pair("track_name", title).append_pair("artist_name", artist);
    if let Some(lines) = best(fetch::<Vec<Record>>(&client, search).await?, duration) {
        return Ok(Some(lines));
    }

    let loose = format!("{} {}", clean_title(title), clean_artist(artist));
    let mut search = url::Url::parse(&format!("{API}/search"))?;
    search.query_pairs_mut().append_pair("q", loose.trim());
    Ok(best(fetch::<Vec<Record>>(&client, search).await?, duration))
}

fn client() -> Result<reqwest::Client, Error> {
    // reqwest is built without a crypto provider; use ring, like the updater does.
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    Ok(reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .build()?)
}

/// GET a JSON document from LRCLIB; Ok(None) for 404 (not found).
async fn fetch<T: for<'de> Deserialize<'de>>(client: &reqwest::Client, url: url::Url) -> Result<Option<T>, Error> {
    let mut response = client.get(url).send().await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    response = response.error_for_status()?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if bytes.len() + chunk.len() > MAX_RESPONSE {
            return Err("lyrics response too large".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(Some(serde_json::from_slice(&bytes)?))
}

/// The synced result whose length is closest to the song's, if any is close enough.
fn best(records: Option<Vec<Record>>, duration: f64) -> Option<Vec<Line>> {
    records?
        .into_iter()
        .filter_map(|r| {
            let off = (r.duration? - duration).abs();
            (off <= DURATION_SLACK).then_some((off, r.synced_lyrics?))
        })
        .map(|(off, lrc)| (off, parse_lrc(&lrc)))
        .filter(|(_, lines)| !lines.is_empty())
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, lines)| lines)
}

/// Parses LRC: `[mm:ss.xx]text`, possibly several timestamps per line. Tags such as `[ar:...]`
/// are skipped. Lines come back sorted by time.
fn parse_lrc(lrc: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    for raw in lrc.lines() {
        let mut rest = raw.trim();
        let mut times = Vec::new();
        while let Some(after) = rest.strip_prefix('[') {
            let Some((tag, tail)) = after.split_once(']') else { break };
            match timestamp(tag) {
                Some(t) => times.push(t),
                // A metadata tag such as [ar:...], or text that starts with "["
                None => break,
            }
            rest = tail;
        }
        let text: String = rest.trim().chars().take(500).collect();
        for t in times {
            lines.push(Line { t, text: text.clone() });
        }
        if lines.len() >= MAX_LINES {
            break;
        }
    }
    lines.sort_by(|a, b| a.t.total_cmp(&b.t));
    // All-empty means instrumental or broken; treat as none.
    if lines.iter().all(|l| l.text.is_empty()) {
        lines.clear();
    }
    lines
}

/// "mm:ss", "mm:ss.xx" or "mm:ss:xx" → seconds
fn timestamp(tag: &str) -> Option<f64> {
    let (m, s) = tag.split_once(':')?;
    let minutes: u32 = m.trim().parse().ok()?;
    let s = s.trim().replacen(':', ".", 1);
    let seconds: f64 = s.parse().ok()?;
    (seconds.is_finite() && (0.0..60.0).contains(&seconds)).then(|| minutes as f64 * 60.0 + seconds)
}

/// Removes "(Official Video)", "[Lyrics]", "| Topic" and similar from video titles.
fn clean_title(title: &str) -> String {
    const NOISE: [&str; 10] = [
        "official", "video", "audio", "lyric", "visualizer", "mv", "hd", "4k", "remaster", "live",
    ];
    let mut out = String::new();
    let mut depth = 0;
    let mut group = String::new();
    for c in title.chars() {
        match c {
            '(' | '[' => {
                depth += 1;
                group.clear();
            }
            ')' | ']' if depth > 0 => {
                depth -= 1;
                let lower = group.to_lowercase();
                if !NOISE.iter().any(|n| lower.contains(n)) {
                    out.push_str(&format!("({group})"));
                }
            }
            _ if depth > 0 => group.push(c),
            _ => out.push(c),
        }
    }
    out.split('|').next().unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// "ArtistVEVO", "Artist - Topic" → "Artist"
fn clean_artist(artist: &str) -> String {
    let a = artist.trim();
    let a = a.strip_suffix(" - Topic").unwrap_or(a);
    let a = a.strip_suffix("VEVO").unwrap_or(a);
    a.trim().to_string()
}

fn clip(s: &str) -> String {
    s.trim().chars().take(300).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lrc() {
        let lines = parse_lrc("[ar: Someone]\n[00:01.50]Hello\n[00:10.00][01:02.25]Again\n[00:05.00]\nnot a lyric");
        let got: Vec<(f64, &str)> = lines.iter().map(|l| (l.t, l.text.as_str())).collect();
        assert_eq!(got, vec![(1.5, "Hello"), (5.0, ""), (10.0, "Again"), (62.25, "Again")]);
        assert!(parse_lrc("[00:01.00]\n[00:02.00]").is_empty());
        assert_eq!(timestamp("03:07:50"), Some(187.5));
        assert_eq!(timestamp("ar: x"), None);
    }

    #[test]
    fn picks_closest_synced_result() {
        let rec = |d: f64, s: Option<&str>| Record { duration: Some(d), synced_lyrics: s.map(String::from) };
        let records = vec![rec(200.0, Some("[00:01.00]far")), rec(181.0, Some("[00:01.00]near")), rec(180.0, None)];
        assert_eq!(best(Some(records), 180.0).unwrap()[0].text, "near");
        assert!(best(Some(vec![rec(190.0, Some("[00:01.00]x"))]), 180.0).is_none());
    }

    #[test]
    fn cleans_video_titles() {
        assert_eq!(clean_title("Song Name (Official Music Video) [4K]"), "Song Name");
        assert_eq!(clean_title("Song (feat. Someone) | Topic"), "Song (feat. Someone)");
        assert_eq!(clean_artist("ArtistVEVO"), "Artist");
        assert_eq!(clean_artist("Artist - Topic"), "Artist");
    }

    #[test]
    #[ignore = "needs the network"]
    fn finds_real_lyrics() {
        let found = tauri::async_runtime::block_on(lookup("Bohemian Rhapsody", "Queen", None, 355.0)).unwrap();
        assert!(found.is_some_and(|l| l.len() > 20));
    }
}
