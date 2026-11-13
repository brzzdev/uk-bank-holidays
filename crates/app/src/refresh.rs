use std::path::PathBuf;
use std::thread;
use std::time::SystemTime;

use bank_holidays::{Calendar, Holidays, load, load_cached};
use chrono::Datelike as _;
use gpui_kit::Context;
use gpui_kit::base::async_util::unbounded;

use crate::{BankHolidays, clock_today};

impl BankHolidays {
    /// Does nothing while a fetch is running, since its result redraws anyway.
    pub(crate) fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.fetching {
            return;
        }
        let Some(home) = self.home.clone() else {
            return;
        };
        let cached = load_cached(&home, SystemTime::now());
        self.replace_holidays(cached.holidays, cx);
        if cached.stale {
            self.fetch(home, cx);
        }
    }

    /// A failed fetch says nothing.
    fn fetch(&mut self, home: PathBuf, cx: &mut Context<Self>) {
        self.fetching = true;
        let (sender, receiver) = unbounded();
        thread::spawn(move || {
            let _ = sender.send_blocking(load(&home, SystemTime::now()));
        });
        cx.spawn(async move |this, cx| {
            let loaded = receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                this.fetching = false;
                if let Ok(loaded) = loaded {
                    this.replace_holidays(loaded.holidays, cx);
                }
            });
        })
        .detach();
    }

    /// Keeps the division and selected day.
    fn replace_holidays(&mut self, holidays: Holidays, cx: &mut Context<Self>) {
        if !supersedes(&holidays, &self.holidays) {
            return;
        }
        self.holidays = holidays.with_predictions(clock_today().year());
        self.selection = self
            .selection
            .kept_in(&Calendar::new(&self.holidays, self.division));
        self.scroll_to_selected_row();
        cx.notify();
    }
}

/// Older data, such as the cache left behind when a fetch couldn't be written, never replaces newer.
fn supersedes(incoming: &Holidays, shown: &Holidays) -> bool {
    incoming.as_of() >= shown.as_of()
}

#[cfg(test)]
mod tests {
    use super::*;

    use bank_holidays::{DatedFeed, merge, snapshot};
    use chrono::Days;

    #[test]
    fn data_as_of_an_older_date_never_replaces_newer_data() {
        let snapshot_only = merge(&snapshot(), None);
        let fetched = DatedFeed {
            as_of: snapshot().as_of + Days::new(10),
            feed: snapshot().feed,
        };
        let fetched = merge(&snapshot(), Some(&fetched));
        assert!(!supersedes(&snapshot_only, &fetched));
        assert!(supersedes(&fetched, &snapshot_only));
        assert!(supersedes(&fetched, &fetched));
    }
}
