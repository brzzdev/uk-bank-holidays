use std::path::PathBuf;
use std::thread;
use std::time::SystemTime;

use bank_holidays::{Calendar, Holidays, load, load_cached};
use chrono::Datelike as _;
use gpui_kit::Context;
use gpui_kit::base::async_util::unbounded;

use crate::{BankHolidays, clock_today};

impl BankHolidays {
    /// Redraws from the cache, then fetches on a thread when the cache is stale. Does nothing while
    /// a fetch is running, since its result redraws anyway. A cache older than the data shown, as
    /// when a fetch succeeded but couldn't be written, doesn't replace it.
    pub(crate) fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.fetching {
            return;
        }
        let Some(home) = self.home.clone() else {
            return;
        };
        let cached = load_cached(&home, SystemTime::now());
        if cached.holidays.as_of() >= self.holidays.as_of() {
            self.replace_holidays(cached.holidays, cx);
        }
        if cached.stale {
            self.fetch(home, cx);
        }
    }

    /// A failed fetch leaves the data as it was and says nothing.
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

    /// Keeps the division and selected day, moving the selection to the nearest year the data covers.
    fn replace_holidays(&mut self, holidays: Holidays, cx: &mut Context<Self>) {
        self.holidays = holidays.with_predictions(clock_today().year());
        self.selection = self
            .selection
            .kept_in(&Calendar::new(&self.holidays, self.division));
        cx.notify();
    }
}
