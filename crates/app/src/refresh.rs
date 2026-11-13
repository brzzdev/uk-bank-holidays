use std::thread;
use std::time::SystemTime;

use bank_holidays::{Calendar, Error, Holidays, load, load_cached};
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
        let Some(home) = &self.home else {
            return;
        };
        let cached = load_cached(home, SystemTime::now());
        if replaces(&cached.holidays, &self.holidays) {
            self.replace_holidays(cached.holidays, cx);
        }
        if cached.stale {
            self.fetch(cx);
        }
    }

    /// A failed fetch leaves the data shown as it was. One whose cache write alone failed still
    /// brings current data.
    pub(crate) fn fetch(&mut self, cx: &mut Context<Self>) {
        let Some(home) = self.home.clone() else {
            return;
        };
        self.fetching = true;
        let (sender, receiver) = unbounded();
        thread::spawn(move || {
            let _ = sender.send_blocking(load(&home, SystemTime::now()));
        });
        cx.spawn(async move |this, cx| {
            let loaded = receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                this.fetching = false;
                if let Ok(loaded) = loaded
                    && matches!(loaded.failure, None | Some(Error::CacheWrite(_)))
                {
                    this.replace_holidays(loaded.holidays, cx);
                }
            });
        })
        .detach();
    }

    /// Moves the selection into the new data's years if it fell outside them.
    fn replace_holidays(&mut self, holidays: Holidays, cx: &mut Context<Self>) {
        self.holidays = holidays.with_predictions(clock_today().year());
        let selection = self
            .selection
            .kept_in(&Calendar::new(&self.holidays, self.division));
        if selection.date != self.selection.date {
            self.selection = selection;
            self.scroll_to_selected_row();
        }
        cx.notify();
    }
}

/// An unchanged cache would only redraw, and an older one is what a fetch that couldn't be
/// written leaves behind.
fn replaces(cached: &Holidays, shown: &Holidays) -> bool {
    cached.as_of() > shown.as_of()
}

#[cfg(test)]
mod tests {
    use super::*;

    use bank_holidays::{DatedFeed, merge, snapshot};
    use chrono::Days;

    #[test]
    fn only_a_cache_newer_than_the_data_shown_replaces_it() {
        let older = merge(&snapshot(), None);
        let newer_feed = DatedFeed {
            as_of: snapshot().as_of + Days::new(10),
            feed: snapshot().feed,
        };
        let newer = merge(&snapshot(), Some(&newer_feed));
        assert!(replaces(&newer, &older));
        assert!(!replaces(&older, &newer));
        assert!(!replaces(&newer, &newer));
    }
}
