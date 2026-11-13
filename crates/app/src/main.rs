mod detail;
mod grid;
mod menus;
mod selection;
mod sidebar;

use std::process;
use std::time::Duration;

use bank_holidays::{Calendar, Division, Holidays, merge, snapshot};
use chrono::{Datelike, Local, NaiveDate, TimeDelta};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{
    ActiveTheme, Disableable as _, IconName, Sizable as _, Theme, TitleBar, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Action, App, AppContext, Bounds, ClickEvent, Context, Div, FocusHandle,
    InteractiveElement as _, IntoElement, KeyBinding, ParentElement, QuitMode, Render,
    ScrollHandle, Styled, Window, WindowBounds, WindowOptions, actions, application, assets, div,
    init, open_window, px, size,
};

use crate::menus::{Close, Hide, HideOthers, Minimise, Quit, menus, on_app_commands};
use crate::selection::{Move, Selection};

actions!(
    bank_holidays,
    [
        NextBankHoliday,
        NextDay,
        NextMonth,
        NextWeek,
        NextYear,
        PreviousBankHoliday,
        PreviousDay,
        PreviousMonth,
        PreviousWeek,
        PreviousYear,
        ShowEnglandAndWales,
        ShowNorthernIreland,
        ShowScotland,
        Today,
    ]
);

/// How often the clock is read for a new day.
const DATE_CHECK_INTERVAL: Duration = Duration::from_secs(60);
/// The division tabs in order, with their labels.
const DIVISIONS: [(Division, &str); 3] = [
    (Division::EnglandAndWales, "England and Wales"),
    (Division::Scotland, "Scotland"),
    (Division::NorthernIreland, "Northern Ireland"),
];
/// "Thursday 25 December 2025"
const FULL_DATE: &str = "%A %-d %B %Y";
const WINDOW_HEIGHT: f32 = 600.;
const WINDOW_WIDTH: f32 = 880.;

struct BankHolidays {
    division: Division,
    focus: FocusHandle,
    holiday_list: ScrollHandle,
    holidays: Holidays,
    selection: Selection,
}

impl BankHolidays {
    fn new(today: NaiveDate, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Theme::sync_system_appearance(Some(window), cx);
        cx.observe_window_appearance(window, |_, window, cx| {
            Theme::sync_system_appearance(Some(window), cx);
        })
        .detach();
        Self::redraw_on_a_new_day(today, cx);
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        // The list can only scroll once its first frame has measured it.
        cx.on_next_frame(window, |this, _, cx| {
            this.scroll_to_selected_row();
            cx.notify();
        });
        Self {
            division: Division::EnglandAndWales,
            focus,
            holiday_list: ScrollHandle::new(),
            holidays: merge(&snapshot(), None).with_predictions(today.year()),
            selection: Selection::new(today),
        }
    }

    fn redraw_on_a_new_day(mut shown: NaiveDate, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(DATE_CHECK_INTERVAL).await;
                let now = clock_today();
                if now == shown {
                    continue;
                }
                shown = now;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    return;
                }
            }
        })
        .detach();
    }

    fn go(&mut self, step: Move, cx: &mut Context<Self>) {
        let calendar = Calendar::new(&self.holidays, self.division);
        let Some(selection) = self.selection.moved(step, &calendar) else {
            return;
        };
        self.selection = selection;
        self.scroll_to_selected_row();
        cx.notify();
    }

    fn show(&mut self, division: Division, cx: &mut Context<Self>) {
        self.division = division;
        self.scroll_to_selected_row();
        cx.notify();
    }

    /// Scrolls the selected bank holiday's row into view, only as far as it's off-screen.
    fn scroll_to_selected_row(&self) {
        let date = self.selection.date;
        let row = self
            .holidays
            .year(self.division, date.year())
            .and_then(|holidays| holidays.iter().position(|holiday| holiday.date == date));
        if let Some(row) = row {
            self.holiday_list.scroll_to_item(row);
        }
    }

    fn select_on_click(
        date: NaiveDate,
        cx: &mut Context<Self>,
    ) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        cx.listener(move |this, _, _, cx| this.go(Move::To(date), cx))
    }

    /// Greyed when the move leaves the available year range.
    fn step_button(
        &self,
        id: &'static str,
        icon: IconName,
        step: Move,
        calendar: &Calendar,
        cx: &mut Context<Self>,
    ) -> Button {
        Button::new(id)
            .ghost()
            .small()
            .tab_stop(false)
            .icon(icon)
            .disabled(self.selection.moved(step, calendar).is_none())
            .on_click(cx.listener(move |this, _, _, cx| this.go(step, cx)))
    }

    /// Registers each move command only while its move can be made.
    fn on_commands(
        &self,
        root: Div,
        calendar: &Calendar,
        today: NaiveDate,
        cx: &mut Context<Self>,
    ) -> Div {
        root.map(|root| self.on_move::<NextBankHoliday>(root, Move::NextBankHoliday, calendar, cx))
            .map(|root| self.on_move::<NextDay>(root, Move::By(TimeDelta::days(1)), calendar, cx))
            .map(|root| self.on_move::<NextMonth>(root, Move::Months(1), calendar, cx))
            .map(|root| self.on_move::<NextWeek>(root, Move::By(TimeDelta::weeks(1)), calendar, cx))
            .map(|root| self.on_move::<NextYear>(root, Move::Years(1), calendar, cx))
            .map(|root| {
                self.on_move::<PreviousBankHoliday>(root, Move::PreviousBankHoliday, calendar, cx)
            })
            .map(|root| {
                self.on_move::<PreviousDay>(root, Move::By(TimeDelta::days(-1)), calendar, cx)
            })
            .map(|root| self.on_move::<PreviousMonth>(root, Move::Months(-1), calendar, cx))
            .map(|root| {
                self.on_move::<PreviousWeek>(root, Move::By(TimeDelta::weeks(-1)), calendar, cx)
            })
            .map(|root| self.on_move::<PreviousYear>(root, Move::Years(-1), calendar, cx))
            .map(|root| Self::on_show::<ShowEnglandAndWales>(root, Division::EnglandAndWales, cx))
            .map(|root| Self::on_show::<ShowNorthernIreland>(root, Division::NorthernIreland, cx))
            .map(|root| Self::on_show::<ShowScotland>(root, Division::Scotland, cx))
            .when(
                self.selection.moved(Move::To(today), calendar).is_some(),
                |root| {
                    root.on_action(cx.listener(|this, _: &Today, _, cx| {
                        this.go(Move::To(clock_today()), cx);
                    }))
                },
            )
    }

    fn on_move<A: Action>(
        &self,
        root: Div,
        step: Move,
        calendar: &Calendar,
        cx: &mut Context<Self>,
    ) -> Div {
        if self.selection.moved(step, calendar).is_none() {
            return root;
        }
        root.on_action(cx.listener(move |this, _: &A, _, cx| this.go(step, cx)))
    }

    fn on_show<A: Action>(root: Div, division: Division, cx: &mut Context<Self>) -> Div {
        root.on_action(cx.listener(move |this, _: &A, _, cx| this.show(division, cx)))
    }

    fn title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = DIVISIONS
            .iter()
            .position(|&(division, _)| division == self.division)
            .expect("every division has a tab");
        TitleBar::new().child(div()).child(
            TabBar::new("divisions")
                .segmented()
                .mr_2()
                .selected_index(selected)
                .on_click(cx.listener(|this, &index: &usize, _, cx| {
                    this.show(DIVISIONS[index].0, cx);
                }))
                .children(DIVISIONS.map(|(_, label)| Tab::new().label(label))),
        )
    }
}

impl Render for BankHolidays {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let today = clock_today();
        let calendar = Calendar::new(&self.holidays, self.division);
        let root = v_flex()
            .track_focus(&self.focus)
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.title_bar(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.sidebar(&calendar, today, cx))
                    .child(self.month_pane(&calendar, today, cx)),
            );
        self.on_commands(root, &calendar, today, cx)
    }
}

fn main() {
    application()
        .with_assets(assets::Assets)
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(|cx| {
            init(cx);
            cx.bind_keys(key_bindings());
            on_app_commands(cx);
            cx.set_menus(menus());
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)),
                    cx,
                ))),
                ..TitleBar::window_options()
            };
            let today = clock_today();
            if let Err(error) = open_window(options, cx, |window, cx| {
                cx.new(|cx| BankHolidays::new(today, window, cx))
            }) {
                eprintln!("bank-holidays-app: couldn't open a window: {error}");
                process::exit(1);
            }
        });
}

fn key_bindings() -> [KeyBinding; 19] {
    [
        KeyBinding::new("alt-cmd-h", HideOthers, None),
        KeyBinding::new("alt-left", PreviousBankHoliday, None),
        KeyBinding::new("alt-right", NextBankHoliday, None),
        KeyBinding::new("cmd-1", ShowEnglandAndWales, None),
        KeyBinding::new("cmd-2", ShowScotland, None),
        KeyBinding::new("cmd-3", ShowNorthernIreland, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("cmd-left", PreviousMonth, None),
        KeyBinding::new("cmd-m", Minimise, None),
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-right", NextMonth, None),
        KeyBinding::new("cmd-shift-left", PreviousYear, None),
        KeyBinding::new("cmd-shift-right", NextYear, None),
        KeyBinding::new("cmd-t", Today, None),
        KeyBinding::new("cmd-w", Close, None),
        KeyBinding::new("down", NextWeek, None),
        KeyBinding::new("left", PreviousDay, None),
        KeyBinding::new("right", NextDay, None),
        KeyBinding::new("up", PreviousWeek, None),
    ]
}

fn clock_today() -> NaiveDate {
    Local::now().date_naive()
}

fn first_of_month(date: NaiveDate) -> NaiveDate {
    date.with_day(1).expect("every month has a first day")
}
