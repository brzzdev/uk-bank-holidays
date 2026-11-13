use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
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

/// The snapshot merged with the feed cached under `home`, read without a request.
#[derive(Debug)]
pub struct Cached {
    pub holidays: Holidays,
    /// Whether the cache is missing, a day old, or dated in the future.
    pub stale: bool,
}

#[must_use]
pub fn load_cached(home: &Path, now: SystemTime) -> Cached {
    let cached = read(&cache_directory(home).join(FEED_FILE));
    Cached {
        holidays: merge(&snapshot(), cached.as_ref().map(|cached| &cached.feed)),
        stale: !cached
            .as_ref()
            .is_some_and(|cached| is_fresh(cached.fetched, now)),
    }
}

/// Merges the snapshot with the feed cached under `home`, first refreshing the cache from gov.uk
/// when it is stale.
#[must_use]
pub fn load(home: &Path, now: SystemTime) -> Loaded {
    load_with(home, now, || fetch(FEED_URL, FETCH_TIMEOUT))
}

fn load_with(
    home: &Path,
    now: SystemTime,
    fetch: impl FnOnce() -> Result<Vec<u8>, Error>,
) -> Loaded {
    let directory = cache_directory(home);
    let cached = read(&directory.join(FEED_FILE));
    if let Some(cached) = &cached
        && is_fresh(cached.fetched, now)
    {
        return with_snapshot(Some(&cached.feed), None);
    }
    let fetched = fetch().and_then(|bytes| Ok((Feed::parse(&bytes)?, bytes)));
    let (feed, bytes) = match fetched {
        Ok(fetched) => fetched,
        Err(error) => {
            return with_snapshot(cached.as_ref().map(|cached| &cached.feed), Some(error));
        }
    };
    let failure = write(&directory, &bytes).err().map(Error::CacheWrite);
    let feed = DatedFeed {
        as_of: utc_date(now),
        feed,
    };
    with_snapshot(Some(&feed), failure)
}

fn with_snapshot(cache: Option<&DatedFeed>, failure: Option<Error>) -> Loaded {
    Loaded {
        failure,
        holidays: merge(&snapshot(), cache),
    }
}

fn cache_directory(home: &Path) -> PathBuf {
    home.join("Library/Caches").join(BUNDLE_ID)
}

/// The cached feed, and its modification time, which is when it was fetched.
struct CachedFeed {
    feed: DatedFeed,
    fetched: SystemTime,
}

fn read(path: &Path) -> Option<CachedFeed> {
    let mut file = File::open(path).ok()?;
    let fetched = file
        .metadata()
        .and_then(|metadata| metadata.modified())
        .ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    let feed = Feed::parse(&bytes).ok()?;
    Some(CachedFeed {
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

/// The response body exactly as sent.
fn fetch(url: &str, timeout: Duration) -> Result<Vec<u8>, Error> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .into();
    agent
        .get(url)
        .call()
        .and_then(|mut response| response.body_mut().read_to_vec())
        .map_err(Error::Fetch)
}

/// Replaces the cached feed through a uniquely named temporary file, so a reader never sees half
/// of it.
fn write(directory: &Path, bytes: &[u8]) -> io::Result<()> {
    static WRITES: AtomicU64 = AtomicU64::new(0);
    fs::create_dir_all(directory)?;
    let temporary = directory.join(format!(
        ".{FEED_FILE}.{}.{}",
        process::id(),
        WRITES.fetch_add(1, Ordering::Relaxed)
    ));
    let written = fs::write(&temporary, bytes)
        .and_then(|()| fs::rename(&temporary, directory.join(FEED_FILE)));
    if written.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    written
}

/// Dates are UTC, as the snapshot's capture date is.
fn utc_date(time: SystemTime) -> NaiveDate {
    DateTime::<Utc>::from(time).date_naive()
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread;

    use tempfile::TempDir;

    use super::*;
    use crate::Division;

    const DAY: Duration = Duration::from_hours(24);
    const SECOND: Duration = Duration::from_secs(1);
    const TEST_TIMEOUT: Duration = Duration::from_millis(100);

    /// A feed giving every division one bank holiday, `title`, on 1 Jan 2099.
    fn feed_json(title: &str) -> serde_json::Value {
        let events = serde_json::json!({
            "events": [{"title": title, "date": "2099-01-01", "notes": "", "bunting": true}],
        });
        serde_json::json!({
            "england-and-wales": events,
            "northern-ireland": events,
            "scotland": events,
        })
    }

    fn feed(title: &str) -> Vec<u8> {
        feed_json(title).to_string().into_bytes()
    }

    fn feed_path(home: &TempDir) -> PathBuf {
        cache_directory(home.path()).join(FEED_FILE)
    }

    fn cache(home: &TempDir, bytes: &[u8], fetched: SystemTime) {
        let path = feed_path(home);
        fs::create_dir_all(cache_directory(home.path())).expect("the cache directory is creatable");
        fs::write(&path, bytes).expect("the cache is writable");
        File::options()
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

    /// Serves one request with `response`, then holds the connection open past the test timeout.
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

    fn http_response(status: &str, body: &[u8]) -> Vec<u8> {
        let mut response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
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

    fn name_in_2099(loaded: &Loaded) -> Option<&str> {
        loaded
            .holidays
            .year(Division::EnglandAndWales, 2099)
            .map(|holidays| holidays[0].name.as_str())
    }

    #[test]
    fn a_missing_cache_is_filled_with_the_response_bytes_exactly() {
        let home = TempDir::new().expect("a temp dir is creatable");
        let body = feed("Fetched Day");
        let url = serve(http_response("200 OK", &body));
        let loaded = load_with(home.path(), later(), || fetch(&url, FETCH_TIMEOUT));
        assert!(loaded.failure.is_none(), "{:?}", loaded.failure);
        assert_eq!(name_in_2099(&loaded), Some("Fetched Day"));
        assert_eq!(fs::read(feed_path(&home)).expect("the cache exists"), body);
    }

    #[test]
    fn a_fresh_cache_is_used_without_a_request() {
        let home = TempDir::new().expect("a temp dir is creatable");
        let now = later();
        cache(&home, &feed("Cached Day"), now - DAY + SECOND);
        let loaded = load_with(home.path(), now, || {
            panic!("a fresh cache makes no request")
        });
        assert!(loaded.failure.is_none(), "{:?}", loaded.failure);
        assert_eq!(name_in_2099(&loaded), Some("Cached Day"));
    }

    #[test]
    fn a_corrupt_cache_counts_as_absent() {
        let home = TempDir::new().expect("a temp dir is creatable");
        cache(&home, b"{\"england-and-wales\":", later());
        let loaded = load_with(home.path(), later(), || Ok(feed("Fetched Day")));
        assert!(loaded.failure.is_none(), "{:?}", loaded.failure);
        assert_eq!(name_in_2099(&loaded), Some("Fetched Day"));
    }

    #[test]
    fn an_unwritable_cache_still_gives_the_fetched_feed() {
        let home = TempDir::new().expect("a temp dir is creatable");
        let directory = cache_directory(home.path());
        fs::create_dir_all(directory.parent().expect("the cache is in Caches"))
            .expect("Caches is creatable");
        fs::write(&directory, b"").expect("a file can block the cache directory");
        let loaded = load_with(home.path(), later(), || Ok(feed("Fetched Day")));
        assert!(matches!(loaded.failure, Some(Error::CacheWrite(_))));
        assert_eq!(name_in_2099(&loaded), Some("Fetched Day"));
    }

    /// Asserts a fetch failing as `expected` leaves a stale cache exactly as it was and uses it.
    fn assert_failed_fetch_keeps_the_cache(
        fetch: impl FnOnce() -> Result<Vec<u8>, Error>,
        expected: fn(&Error) -> bool,
    ) {
        let home = TempDir::new().expect("a temp dir is creatable");
        let cached = feed("Cached Day");
        cache(&home, &cached, later() - 2 * DAY);
        let path = feed_path(&home);
        let before = modified(&path);
        let loaded = load_with(home.path(), later(), fetch);
        assert!(
            loaded.failure.as_ref().is_some_and(expected),
            "{:?}",
            loaded.failure
        );
        assert_eq!(name_in_2099(&loaded), Some("Cached Day"));
        assert_eq!(fs::read(&path).expect("the cache remains"), cached);
        assert_eq!(modified(&path), before);
        let files = fs::read_dir(cache_directory(home.path()))
            .expect("the cache directory remains")
            .count();
        assert_eq!(files, 1, "no temporary file is left behind");
    }

    #[test]
    fn a_timed_out_fetch_keeps_the_cache() {
        let url = serve(Vec::new());
        assert_failed_fetch_keeps_the_cache(
            || fetch(&url, TEST_TIMEOUT),
            |error| matches!(error, Error::Fetch(ureq::Error::Timeout(_))),
        );
    }

    #[test]
    fn an_http_error_keeps_the_cache() {
        let url = serve(http_response("503 Service Unavailable", b""));
        assert_failed_fetch_keeps_the_cache(
            || fetch(&url, TEST_TIMEOUT),
            |error| matches!(error, Error::Fetch(ureq::Error::StatusCode(503))),
        );
    }

    #[test]
    fn an_unparseable_body_keeps_the_cache() {
        assert_failed_fetch_keeps_the_cache(
            || Ok(b"<html>".to_vec()),
            |error| matches!(error, Error::Feed(_)),
        );
    }

    #[test]
    fn a_body_missing_a_division_keeps_the_cache() {
        let mut body = feed_json("Fetched Day");
        body.as_object_mut()
            .expect("the feed is an object")
            .remove("scotland");
        assert_failed_fetch_keeps_the_cache(
            || Ok(body.to_string().into_bytes()),
            |error| matches!(error, Error::Feed(_)),
        );
    }

    #[test]
    fn a_division_with_no_events_keeps_the_cache() {
        let mut body = feed_json("Fetched Day");
        body["scotland"]["events"] = serde_json::json!([]);
        assert_failed_fetch_keeps_the_cache(
            || Ok(body.to_string().into_bytes()),
            |error| matches!(error, Error::EmptyDivision(Division::Scotland)),
        );
    }

    #[test]
    fn a_cache_is_fresh_until_it_is_a_day_old() {
        let fetched = SystemTime::UNIX_EPOCH + 1000 * DAY;
        assert!(is_fresh(fetched, fetched));
        assert!(is_fresh(fetched, fetched + DAY - SECOND));
        assert!(!is_fresh(fetched, fetched + DAY));
    }

    #[test]
    fn a_missing_cache_reads_as_the_snapshot_and_stale() {
        let home = TempDir::new().expect("a temp dir is creatable");
        let cached = load_cached(home.path(), later());
        assert!(cached.stale);
        assert_eq!(cached.holidays.as_of(), snapshot().as_of);
    }

    #[test]
    fn a_fresh_cache_reads_as_of_its_modification_date_and_not_stale() {
        let home = TempDir::new().expect("a temp dir is creatable");
        let now = later();
        let fetched = now - DAY + SECOND;
        cache(&home, &feed("Cached Day"), fetched);
        let cached = load_cached(home.path(), now);
        assert!(!cached.stale);
        assert_eq!(cached.holidays.as_of(), utc_date(fetched));
        assert_eq!(
            cached
                .holidays
                .year(Division::EnglandAndWales, 2099)
                .map(|holidays| holidays[0].name.as_str()),
            Some("Cached Day"),
        );
    }

    #[test]
    fn a_cache_from_the_future_is_stale() {
        let now = SystemTime::UNIX_EPOCH + 1000 * DAY;
        assert!(!is_fresh(now + SECOND, now));
    }
}
