use std::path::Path;
use std::thread;
use std::time::SystemTime;

use bank_holidays::{Cached, Calendar, Holidays, load, load_cached, merge, snapshot};
use chrono::Datelike as _;
use futures_channel::oneshot;
use gpui_kit::Context;

use crate::{BankHolidays, clock_today};

impl BankHolidays {
    /// Redraws from the cache, then fetches on a thread when the cache is stale. Does nothing while
    /// a fetch is running, since its result redraws anyway.
    pub(crate) fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.fetching {
            return;
        }
        let cached = read_cache(self.home.as_deref());
        self.replace_holidays(cached.holidays, cx);
        if cached.stale {
            self.fetch(cx);
        }
    }

    /// A failed fetch leaves the data as it was and says nothing.
    pub(crate) fn fetch(&mut self, cx: &mut Context<Self>) {
        let Some(home) = self.home.clone() else {
            return;
        };
        self.fetching = true;
        let (sender, receiver) = oneshot::channel();
        thread::spawn(move || {
            let _ = sender.send(load(&home, SystemTime::now()));
        });
        cx.spawn(async move |this, cx| {
            let loaded = receiver.await;
            let _ = this.update(cx, |this, cx| {
                this.fetching = false;
                if let Ok(loaded) = loaded {
                    this.replace_holidays(loaded.holidays, cx);
                }
            });
        })
        .detach();
    }

    /// Keeps the division and selected day, moving the selection to the nearest year left.
    fn replace_holidays(&mut self, holidays: Holidays, cx: &mut Context<Self>) {
        self.holidays = holidays.with_predictions(clock_today().year());
        self.selection = self
            .selection
            .kept_in(&Calendar::new(&self.holidays, self.division));
        cx.notify();
    }
}

/// The snapshot alone when there's no home directory to hold a cache.
pub(crate) fn read_cache(home: Option<&Path>) -> Cached {
    match home {
        Some(home) => load_cached(home, SystemTime::now()),
        None => Cached {
            holidays: merge(&snapshot(), None),
            stale: false,
        },
    }
}
