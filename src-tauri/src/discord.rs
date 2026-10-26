//! Discord Rich Presence ("Listening to ..."). Runs on its own thread so a slow or missing
//! Discord never blocks the app, and only ever talks to the local Discord client over IPC.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use discord_rich_presence::activity::{
    Activity, ActivityType, Assets, Button, StatusDisplayType, Timestamps,
};
use discord_rich_presence::error::Error;
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

use crate::TrackInfo;

/// Application ID from https://discord.com/developers/applications. Its name ("Optimusic") is
/// what Discord shows, and its "logo" art asset is the fallback cover image.
/// Empty turns the feature off; `OPTIMUSIC_DISCORD_CLIENT_ID` overrides it.
const CLIENT_ID: &str = "1564122032537337927";

/// How often to look for Discord when it isn't running (or went away).
const RETRY: Duration = Duration::from_secs(30);

pub enum Msg {
    Track(TrackInfo),
    Enabled(bool),
}

pub fn client_id() -> Option<String> {
    let id = std::env::var("OPTIMUSIC_DISCORD_CLIENT_ID").unwrap_or_else(|_| CLIENT_ID.into());
    (!id.is_empty()).then_some(id)
}

pub fn spawn(client_id: String, enabled: bool) -> Sender<Msg> {
    let (tx, rx) = mpsc::channel();
    let _ = std::thread::Builder::new()
        .name("discord".into())
        .spawn(move || run(rx, client_id, enabled));
    tx
}

fn run(rx: Receiver<Msg>, client_id: String, mut enabled: bool) {
    let mut client: Option<DiscordIpcClient> = None;
    let mut last_attempt: Option<Instant> = None;
    let mut track: Option<TrackInfo> = None;
    // What Discord is showing right now: (track key, end time in ms).
    let mut shown: Option<(String, i64)> = None;

    loop {
        match rx.recv_timeout(RETRY) {
            Ok(Msg::Track(t)) => track = Some(t),
            Ok(Msg::Enabled(e)) => enabled = e,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        let playing = track.as_ref().filter(|t| t.playing && enabled);

        if client.is_none() {
            // Only look for Discord when there's something to show.
            let due = last_attempt.map_or(true, |t| t.elapsed() >= RETRY);
            if playing.is_none() || !due {
                continue;
            }
            last_attempt = Some(Instant::now());
            let mut c = DiscordIpcClient::new(&client_id);
            if c.connect().is_err() {
                continue;
            }
            client = Some(c);
            shown = None;
        }
        let c = client.as_mut().unwrap();

        let result = match playing {
            Some(t) => {
                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
                let start = now.as_millis() as i64 - (t.position.max(0.0) * 1000.0) as i64;
                let end = start + (t.duration * 1000.0) as i64;
                let key = format!("{}\n{}", t.title, t.artist);
                // Discord rate-limits updates, so skip the periodic progress reports unless
                // the track changed or the user seeked.
                if shown.as_ref().is_some_and(|(k, e)| *k == key && (e - end).abs() < 3000) {
                    continue;
                }
                shown = Some((key, end));
                // If Discord rejects it (say, a cover URL it doesn't like), fall back to the
                // plain version: app logo, no button.
                match apply(c, Some(activity(t, start, end, true))) {
                    Ok(false) => apply(c, Some(activity(t, start, end, false))).map(drop),
                    r => r.map(drop),
                }
            }
            None if shown.take().is_some() => apply(c, None).map(drop),
            None => continue,
        };
        if result.is_err() {
            // Discord was closed; look for it again later.
            client = None;
            shown = None;
        }
    }
}

/// Sets (or with `None`, clears) the activity and reads Discord's reply. The library doesn't
/// read replies itself, so without this they'd pile up in the pipe. Returns whether Discord
/// accepted the activity.
fn apply(c: &mut DiscordIpcClient, a: Option<Activity>) -> Result<bool, Error> {
    match a {
        Some(a) => c.set_activity(a)?,
        None => c.clear_activity()?,
    }
    let (_, reply) = c.recv()?;
    Ok(reply["evt"] != "ERROR")
}

fn activity(t: &TrackInfo, start: i64, end: i64, rich: bool) -> Activity<'_> {
    let mut timestamps = Timestamps::new().start(start);
    if t.duration > 0.0 {
        timestamps = timestamps.end(end);
    }
    // Discord rejects image URLs over 256 characters; fall back to the app's own logo.
    let art = t.art.as_deref().filter(|u| rich && u.len() <= 256).unwrap_or("logo");
    let mut assets = Assets::new().large_image(art);
    if let Some(album) = t.album.as_deref().filter(|a| !a.is_empty()) {
        assets = assets.large_text(field(album));
    }
    let mut a = Activity::new()
        .activity_type(ActivityType::Listening)
        .status_display_type(StatusDisplayType::Details)
        .details(field(&t.title))
        .state(field(&t.artist))
        .timestamps(timestamps)
        .assets(assets);
    if let Some(id) = t.video_id.as_deref().filter(|id| rich && !id.is_empty()) {
        let url = format!("https://music.youtube.com/watch?v={id}");
        a = a.buttons(vec![Button::new("Listen on YouTube Music", url)]);
    }
    a
}

/// Discord text fields must be 2–128 characters.
fn field(s: &str) -> String {
    let mut s: String = s.chars().take(128).collect();
    while s.chars().count() < 2 {
        s.push('\u{2800}'); // blank braille cell: invisible, but Discord doesn't trim it
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs the Discord desktop app running: `cargo test -- --ignored discord`
    #[test]
    #[ignore]
    fn discord_accepts_activity() {
        let mut c = DiscordIpcClient::new(client_id().unwrap());
        c.connect().expect("is Discord running?");
        let t = TrackInfo {
            title: "Test Track".into(),
            artist: "Optimusic".into(),
            album: Some("Test Album".into()),
            art: None,
            video_id: Some("dQw4w9WgXcQ".into()),
            duration: 200.0,
            position: 20.0,
            playing: true,
        };
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
        assert!(apply(&mut c, Some(activity(&t, now - 20_000, now + 180_000, true))).unwrap());
        std::thread::sleep(Duration::from_secs(8));
        assert!(apply(&mut c, None).unwrap());
    }
}
