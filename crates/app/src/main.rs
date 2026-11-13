mod sidebar;

use std::process;

use bank_holidays::{Calendar, Division, Holidays, merge, snapshot};
use chrono::{Datelike, Local};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme, Theme, TitleBar, h_flex, v_flex};
use gpui_kit::{
    AppContext, Bounds, Context, IntoElement, ParentElement, QuitMode, Render, Styled, Window,
    WindowBounds, WindowOptions, application, assets, div, init, open_window, px, size,
};

/// The division tabs in order, with their labels.
const DIVISIONS: [(Division, &str); 3] = [
    (Division::EnglandAndWales, "England and Wales"),
    (Division::Scotland, "Scotland"),
    (Division::NorthernIreland, "Northern Ireland"),
];
const WINDOW_HEIGHT: f32 = 600.;
const WINDOW_WIDTH: f32 = 880.;

struct BankHolidays {
    division: Division,
    holidays: Holidays,
    year: i32,
}

impl BankHolidays {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Theme::sync_system_appearance(Some(window), cx);
        cx.observe_window_appearance(window, |_, window, cx| {
            Theme::sync_system_appearance(Some(window), cx);
        })
        .detach();
        let current_year = Local::now().year();
        Self {
            division: Division::EnglandAndWales,
            holidays: merge(&snapshot(), None).with_predictions(current_year),
            year: current_year,
        }
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
                    this.division = DIVISIONS[index].0;
                    cx.notify();
                }))
                .children(DIVISIONS.map(|(_, label)| Tab::new().label(label))),
        )
    }
}

impl Render for BankHolidays {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let today = Local::now().date_naive();
        let calendar = Calendar::new(&self.holidays, self.division);
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.title_bar(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.sidebar(&calendar, today, cx))
                    .child(div().flex_1()),
            )
    }
}

fn main() {
    application()
        .with_assets(assets::Assets)
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(|cx| {
            init(cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)),
                    cx,
                ))),
                ..TitleBar::window_options()
            };
            if let Err(error) = open_window(options, cx, |window, cx| {
                cx.new(|cx| BankHolidays::new(window, cx))
            }) {
                eprintln!("bank-holidays-app: couldn't open a window: {error}");
                process::exit(1);
            }
        });
}
