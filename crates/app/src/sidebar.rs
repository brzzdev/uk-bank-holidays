use bank_holidays::{BankHoliday, Calendar, Next};
use chrono::{Datelike as _, NaiveDate};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme, IconName, StyledExt as _, Theme, h_flex, v_flex};
use gpui_kit::{
    App, Context, Div, Hsla, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, div, px,
};

use crate::{BankHolidays, FULL_DATE};

const DATE_COLUMN_WIDTH: f32 = 52.;
const DOT_SIZE: f32 = 7.;
const MONTHS_IN_YEAR: i32 = 12;
const SIDEBAR_WIDTH: f32 = 270.;
const YEAR_WIDTH: f32 = 40.;

/// The one mark a row of the list carries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Dot {
    Bridge,
    FourDayWeekend,
    Predicted,
}

impl BankHolidays {
    pub(crate) fn sidebar(
        &self,
        calendar: &Calendar,
        today: NaiveDate,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let holidays = self
            .holidays
            .year(self.division, self.month.year())
            .unwrap_or_default();
        v_flex()
            .w(px(SIDEBAR_WIDTH))
            .flex_none()
            .h_full()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .children(calendar.next(today).map(|next| next_holiday(&next, cx)))
            .child(self.year_stepper(cx))
            .child(
                v_flex()
                    .id("holidays")
                    .flex_1()
                    .min_h_0()
                    .p_1p5()
                    .gap_px()
                    .children(holidays.iter().enumerate().map(|(index, holiday)| {
                        row(holiday, dot(calendar, holiday), today, cx)
                            .id(("holiday", index))
                            .on_click(Self::select_on_click(holiday.date, cx))
                    }))
                    .overflow_y_scrollbar(),
            )
    }

    fn year_stepper(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .justify_center()
            .gap_1()
            .p_1p5()
            .border_b_1()
            .border_color(cx.theme().sidebar_border)
            .child(self.step_button("previous-year", IconName::ChevronLeft, -MONTHS_IN_YEAR, cx))
            .child(
                div()
                    .min_w(px(YEAR_WIDTH))
                    .text_center()
                    .font_semibold()
                    .child(self.month.year().to_string()),
            )
            .child(self.step_button("next-year", IconName::ChevronRight, MONTHS_IN_YEAR, cx))
    }
}

fn dot(calendar: &Calendar, holiday: &BankHoliday) -> Option<Dot> {
    if holiday.predicted {
        return Some(Dot::Predicted);
    }
    if calendar.four_day_weekend(holiday.date).is_some() {
        return Some(Dot::FourDayWeekend);
    }
    if calendar.bridges_joined(holiday.date).next().is_some() {
        return Some(Dot::Bridge);
    }
    None
}

/// A dashed dot takes the row's text colour.
fn dot_mark(dot: Option<Dot>, text: Hsla, theme: &Theme) -> Div {
    let mark = div().flex_none().size(px(DOT_SIZE)).rounded_full();
    match dot {
        None => mark,
        Some(Dot::Bridge) => mark.bg(theme.warning),
        Some(Dot::FourDayWeekend) => mark.bg(theme.red),
        Some(Dot::Predicted) => mark.border_1().border_dashed().border_color(text),
    }
}

fn next_holiday(next: &Next, cx: &App) -> Div {
    let theme = cx.theme();
    let caption = match next.days {
        0 => "today",
        1 => "day to go",
        _ => "days to go",
    };
    h_flex()
        .gap_2()
        .p_3()
        .border_b_1()
        .border_color(theme.sidebar_border)
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(div().text_xs().font_semibold().child("Next bank holiday"))
                .child(
                    div()
                        .text_base()
                        .font_semibold()
                        .text_color(theme.foreground)
                        .child(next.holiday.name.clone()),
                )
                .child(next.holiday.date.format(FULL_DATE).to_string())
                .children(next_marks(next)),
        )
        .child(
            v_flex()
                .flex_none()
                .items_end()
                .child(
                    div()
                        .text_3xl()
                        .font_bold()
                        .text_color(theme.red)
                        .child(next.days.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(caption),
                ),
        )
}

fn next_marks(next: &Next) -> Option<&'static str> {
    match (next.starts_four_day_weekend, next.holiday.predicted) {
        (false, false) => None,
        (false, true) => Some("Predicted"),
        (true, false) => Some("Four-day weekend"),
        (true, true) => Some("Four-day weekend · Predicted"),
    }
}

fn row(holiday: &BankHoliday, dot: Option<Dot>, today: NaiveDate, cx: &App) -> Div {
    let theme = cx.theme();
    let colour = if holiday.date < today {
        theme.muted_foreground
    } else {
        theme.foreground
    };
    h_flex()
        .gap_2()
        .px_2()
        .py_1p5()
        .text_sm()
        .text_color(colour)
        .child(
            div()
                .w(px(DATE_COLUMN_WIDTH))
                .flex_none()
                .text_color(theme.muted_foreground)
                .child(holiday.date.format("%-d %b").to_string()),
        )
        .child(div().flex_1().min_w_0().child(holiday.name.clone()))
        .child(dot_mark(dot, colour, theme))
}

#[cfg(test)]
mod tests {
    use super::*;

    use bank_holidays::{Division, merge, snapshot};

    #[test]
    fn a_row_is_dotted_for_a_prediction_before_a_four_day_weekend_before_a_bridge() {
        let holidays = merge(&snapshot(), None).with_predictions(2026);
        let division = Division::EnglandAndWales;
        let calendar = Calendar::new(&holidays, division);
        let year = |year| holidays.year(division, year).unwrap_or_default();
        let dot_on = |date: &str| {
            let date: NaiveDate = date.parse().expect("test dates are valid");
            let holiday = year(date.year())
                .iter()
                .find(|holiday| holiday.date == date)
                .expect("each test date is a bank holiday");
            dot(&calendar, holiday)
        };
        assert_eq!(dot_on("2025-05-05"), None);
        assert_eq!(dot_on("2025-12-25"), Some(Dot::FourDayWeekend));
        assert_eq!(dot_on("2026-01-01"), Some(Dot::Bridge));
        assert!(
            year(*holidays.years(division).end())
                .iter()
                .all(|holiday| dot(&calendar, holiday) == Some(Dot::Predicted))
        );
    }
}
