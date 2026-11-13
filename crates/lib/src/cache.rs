use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use chrono::{DateTime, NaiveDate, Utc};

use crate::{DatedFeed, Error, Feed, Holidays, merge, snapshot};

/// The app's bundle identifier, which also names the cache directory.
pub const BUNDLE_ID: &str = "dev.brzz.bank-holidays";

const FEED_FILE: &str = "feed.json";
const FEED_URL: &str = "https://www.gov.uk/bank-holidays.json";
const FETCH_TIMEOUT: Duration = Duration::from_secs(5);
const FRESH_FOR: Duration = Duration::from_hours(24);

/// The merged bank holidays, and why refreshing the cache failed if it did.
#[derive(Debug)]
pub struct Loaded {
    pub failure: Option<Error>,
    pub holidays: Holidays,
}

/// Where the feed is fetched from, and how long a fetch may take.
struct Source<'a> {
    timeout: Duration,
    url: &'a str,
}

/// Merges the snapshot with the feed cached under `home`, first refreshing the cache from gov.uk
/// when it is stale.
#[must_use]
pub fn load(home: &Path, now: SystemTime) -> Loaded {
    let source = Source {
        timeout: FETCH_TIMEOUT,
        url: FEED_URL,
    };
    load_from(home, now, &source)
}

fn load_from(home: &Path, now: SystemTime, source: &Source) -> Loaded {
    let directory = home.join("Library/Caches").join(BUNDLE_ID);
    let path = directory.join(FEED_FILE);
    let cached = read(&path);
    let (cache, failure) = match cached {
        Some(cached) if is_fresh(cached.fetched, now) => (Some(cached.feed), None),
        cached => match fetch(source) {
            Ok((bytes, feed)) => {
                let fetched = DatedFeed {
                    as_of: utc_date(now),
                    feed,
                };
                let failure = write(&directory, &path, &bytes)
                    .err()
                    .map(Error::CacheWrite);
                (Some(fetched), failure)
            }
            Err(error) => (cached.map(|cached| cached.feed), Some(error)),
        },
    };
    Loaded {
        failure,
        holidays: merge(&snapshot(), cache.as_ref()),
    }
}

/// The cached feed, and its modification time, which is when it was fetched.
struct Cached {
    feed: DatedFeed,
    fetched: SystemTime,
}

/// The cached feed, or none when it is missing or unreadable.
fn read(path: &Path) -> Option<Cached> {
    let mut file = File::open(path).ok()?;
    let fetched = file
        .metadata()
        .and_then(|metadata| metadata.modified())
        .ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    let feed = Feed::parse(&bytes).ok()?;
    Some(Cached {
        feed: DatedFeed {
            as_of: utc_date(fetched),
            feed,
        },
        fetched,
    })
}

/// A future modification time is stale, so a clock set back can't keep the cache fresh.
fn is_fresh(fetched: SystemTime, now: SystemTime) -> bool {
    now.duration_since(fetched).is_ok_and(|age| age < FRESH_FOR)
}

/// gov.uk's response, exactly as sent, and the feed it parses into.
fn fetch(source: &Source) -> Result<(Vec<u8>, Feed), Error> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(source.timeout))
        .build()
        .into();
    let bytes = agent
        .get(source.url)
        .call()
        .and_then(|mut response| response.body_mut().read_to_vec())
        .map_err(Error::Fetch)?;
    let feed = Feed::parse(&bytes)?;
    Ok((bytes, feed))
}

/// Replaces the cached feed through a uniquely named temporary file, so a reader never sees half.
fn write(directory: &Path, path: &Path, bytes: &[u8]) -> io::Result<()> {
    static WRITES: AtomicU64 = AtomicU64::new(0);
    fs::create_dir_all(directory)?;
    let temporary = directory.join(format!(
        ".{FEED_FILE}.{}.{}",
        process::id(),
        WRITES.fetch_add(1, Ordering::Relaxed)
    ));
    let written = fs::write(&temporary, bytes).and_then(|()| fs::rename(&temporary, path));
    if written.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    written
}

/// The UTC date of a time, matching the snapshot's capture date.
fn utc_date(time: SystemTime) -> NaiveDate {
    DateTime::<Utc>::from(time).date_naive()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::thread;

    use tempfile::TempDir;

    use super::*;
    use crate::Division;

    const DAY: Duration = Duration::from_hours(24);
    const SECOND: Duration = Duration::from_secs(1);
    /// A port nothing listens on, so any request to it fails.
    const UNREACHABLE: &str = "http://127.0.0.1:1/bank-holidays.json";

    /// A feed giving every division one bank holiday, `title`, on 1 Jan 2099.
    fn feed(title: &str) -> Vec<u8> {
        let events = serde_json::json!({
            "events": [{"title": title, "date": "2099-01-01", "notes": "", "bunting": true}],
        });
        serde_json::json!({
            "england-and-wales": events,
            "northern-ireland": events,
            "scotland": events,
        })
        .to_string()
        .into_bytes()
    }

    fn feed_path(home: &TempDir) -> PathBuf {
        home.path()
            .join("Library/Caches")
            .join(BUNDLE_ID)
            .join("feed.json")
    }

    /// Writes `bytes` as the cached feed, fetched at `fetched`.
    fn cache(home: &TempDir, bytes: &[u8], fetched: SystemTime) {
        let path = feed_path(home);
        fs::create_dir_all(path.parent().expect("the feed is in a directory"))
            .expect("the cache directory is creatable");
        fs::write(&path, bytes).expect("the cache is writable");
        fs::File::options()
            .write(true)
            .open(&path)
            .and_then(|file| file.set_modified(fetched))
            .expect("the cache's modification time is settable");
    }

    fn modified(path: &Path) -> SystemTime {
        fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .expect("the cache has a modification time")
    }

    /// Serves one request with `response`, then holds the connection open past the fetch timeout.
    fn serve(response: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a local port is free");
        let address = listener.local_addr().expect("the listener has an address");
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("the client connects");
            let mut request = [0; 4096];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(&response);
            thread::sleep(2 * SECOND);
        });
        format!("http://{address}/bank-holidays.json")
    }

    fn ok(body: &[u8]) -> Vec<u8> {
        let mut response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        response.extend_from_slice(body);
        response
    }

    /// A clock years ahead, so every cache it dates postdates the snapshot.
    fn later() -> SystemTime {
        SystemTime::now() + 1000 * DAY
    }

    fn load_with(home: &TempDir, url: &str) -> Loaded {
        let source = Source {
            timeout: Duration::from_millis(500),
            url,
        };
        load_from(home.path(), later(), &source)
    }

    fn name_in_2099(loaded: &Loaded) -> Option<&str> {
        loaded
            .holidays
            .year(Division::EnglandAndWales, 2099)
            .map(|holidays| holidays[0].name.as_str())
    }

    #[test]
    fn a_missing_cache_is_filled_with_gov_uk_bytes_exactly() {
        let home = TempDir::new().expect("a temp dir is creatable");
        let body = feed("Fetched Day");
        let loaded = load_with(&home, &serve(ok(&body)));
        assert!(loaded.failure.is_none(), "{:?}", loaded.failure);
        assert_eq!(name_in_2099(&loaded), Some("Fetched Day"));
        assert_eq!(fs::read(feed_path(&home)).expect("the cache exists"), body);
    }

    #[test]
    fn a_fresh_cache_is_used_without_a_request() {
        let home = TempDir::new().expect("a temp dir is creatable");
        cache(&home, &feed("Cached Day"), later() - DAY + 60 * SECOND);
        let loaded = load_with(&home, UNREACHABLE);
        assert!(loaded.failure.is_none(), "{:?}", loaded.failure);
        assert_eq!(name_in_2099(&loaded), Some("Cached Day"));
    }

    #[test]
    fn a_corrupt_cache_counts_as_absent() {
        let home = TempDir::new().expect("a temp dir is creatable");
        cache(&home, b"{\"england-and-wales\":", later());
        let loaded = load_with(&home, UNREACHABLE);
        assert!(matches!(loaded.failure, Some(Error::Fetch(_))));
        assert_eq!(name_in_2099(&loaded), None);
    }

    #[test]
    fn an_unwritable_cache_still_gives_the_fetched_feed() {
        let home = TempDir::new().expect("a temp dir is creatable");
        let directory = feed_path(&home);
        let directory = directory.parent().expect("the feed is in a directory");
        fs::create_dir_all(directory.parent().expect("the cache is in Caches"))
            .expect("Caches is creatable");
        fs::write(directory, b"").expect("a file can block the cache directory");
        let loaded = load_with(&home, &serve(ok(&feed("Fetched Day"))));
        assert!(matches!(loaded.failure, Some(Error::CacheWrite(_))));
        assert_eq!(name_in_2099(&loaded), Some("Fetched Day"));
    }

    /// Asserts a fetch answered with `response` fails as `expected`, leaving a stale cache exactly
    /// as it was and using it.
    fn assert_failed_fetch_keeps_the_cache(response: Vec<u8>, expected: fn(&Error) -> bool) {
        let home = TempDir::new().expect("a temp dir is creatable");
        let cached = feed("Cached Day");
        cache(&home, &cached, later() - 2 * DAY);
        let path = feed_path(&home);
        let before = modified(&path);
        let loaded = load_with(&home, &serve(response));
        assert!(
            loaded.failure.as_ref().is_some_and(expected),
            "{:?}",
            loaded.failure
        );
        assert_eq!(name_in_2099(&loaded), Some("Cached Day"));
        assert_eq!(fs::read(&path).expect("the cache remains"), cached);
        assert_eq!(modified(&path), before);
        let files = fs::read_dir(path.parent().expect("the feed is in a directory"))
            .expect("the cache directory remains")
            .count();
        assert_eq!(files, 1, "no temporary file is left behind");
    }

    #[test]
    fn a_timed_out_fetch_keeps_the_cache() {
        assert_failed_fetch_keeps_the_cache(Vec::new(), |error| {
            matches!(error, Error::Fetch(ureq::Error::Timeout(_)))
        });
    }

    #[test]
    fn an_http_error_keeps_the_cache() {
        assert_failed_fetch_keeps_the_cache(
            b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_vec(),
            |error| matches!(error, Error::Fetch(ureq::Error::StatusCode(503))),
        );
    }

    #[test]
    fn an_unparseable_body_keeps_the_cache() {
        assert_failed_fetch_keeps_the_cache(ok(b"<html>"), |error| matches!(error, Error::Feed(_)));
    }

    #[test]
    fn a_body_missing_a_division_keeps_the_cache() {
        let body = br#"{"england-and-wales":{"events":[]},"scotland":{"events":[]}}"#;
        assert_failed_fetch_keeps_the_cache(ok(body), |error| matches!(error, Error::Feed(_)));
    }

    #[test]
    fn a_division_with_no_events_keeps_the_cache() {
        let mut body: serde_json::Value =
            serde_json::from_slice(&feed("Fetched Day")).expect("the feed is JSON");
        body["scotland"]["events"] = serde_json::json!([]);
        assert_failed_fetch_keeps_the_cache(ok(body.to_string().as_bytes()), |error| {
            matches!(error, Error::EmptyDivision(Division::Scotland))
        });
    }

    #[test]
    fn a_cache_is_fresh_until_it_is_a_day_old() {
        let fetched = SystemTime::UNIX_EPOCH + 1000 * DAY;
        assert!(is_fresh(fetched, fetched));
        assert!(is_fresh(fetched, fetched + DAY - SECOND));
        assert!(!is_fresh(fetched, fetched + DAY));
    }

    #[test]
    fn a_cache_from_the_future_is_stale() {
        let now = SystemTime::UNIX_EPOCH + 1000 * DAY;
        assert!(!is_fresh(now + SECOND, now));
    }
}
