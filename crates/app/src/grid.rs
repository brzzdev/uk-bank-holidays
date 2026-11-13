use bank_holidays::{Calendar, Day};
use chrono::{Datelike as _, Days, Months, NaiveDate};
use gpui_kit::component::{ActiveTheme as _, IconName, StyledExt as _, ThemeColor, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Context, Div, Hsla, InteractiveElement as _, IntoElement, ParentElement, Stateful,
    StatefulInteractiveElement as _, Styled, div, pattern_slash, px,
};

use crate::BankHolidays;
use crate::detail::detail;

/// The red behind a four-day weekend's days, as an opacity of `red`.
const FOUR_DAY_WEEKEND_TINT: f32 = 0.12;
/// The amber behind a bridge's days, as an opacity of `warning`.
const BRIDGE_TINT: f32 = 0.12;
/// The bridge's hatch lines, as an opacity of `warning`.
const BRIDGE_HATCH: f32 = 0.35;
const BRIDGE_HATCH_INTERVAL: f32 = 4.;
const BRIDGE_HATCH_WIDTH: f32 = 1.5;
/// A dimmed overflow day's bank holiday disc, as an opacity of `red`.
const OVERFLOW_FILL: f32 = 0.8;

const CELL_HEIGHT: f32 = 42.;
const CELL_RADIUS: f32 = 6.;
const DISC_SIZE: f32 = 30.;
const LEGEND_SWATCH_SIZE: f32 = 10.;
const MONTH_TITLE_WIDTH: f32 = 150.;
const RING_SIZE: f32 = 38.;
const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// The shading behind a day's numeral.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Area {
    Bridge,
    FourDayWeekend,
}

/// The circle a bank holiday's numeral sits in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Disc {
    /// A dashed outline: a predicted bank holiday.
    Dashed,
    Filled,
}

/// How a day's cell is drawn.
struct Look {
    area: Option<Area>,
    disc: Option<Disc>,
    /// A day from a neighbouring month, dimmed.
    overflow: bool,
    selected: bool,
    today: bool,
}

impl BankHolidays {
    pub(crate) fn month_pane(&self, today: NaiveDate, cx: &mut Context<Self>) -> impl IntoElement {
        let calendar = Calendar::new(&self.holidays, self.division);
        let days: Vec<_> = month_days(self.month).collect();
        let weeks: Vec<_> = days
            .chunks(WEEKDAYS.len())
            .enumerate()
            .map(|(week, days)| {
                h_flex()
                    .gap_1()
                    .children(days.iter().enumerate().map(|(weekday, &date)| {
                        let look = Look {
                            area: area(&calendar, date),
                            disc: disc(&calendar, date),
                            overflow: (date.year(), date.month())
                                != (self.month.year(), self.month.month()),
                            selected: date == self.selected,
                            today: date == today,
                        };
                        cell(week * WEEKDAYS.len() + weekday, date, &look, cx).on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.select(date);
                                cx.notify();
                            }),
                        )
                    }))
            })
            .collect();
        let detail = detail(&calendar, self.selected, today);
        let previous = self.step_button("previous-month", IconName::ChevronLeft, -1, cx);
        let next = self.step_button("next-month", IconName::ChevronRight, 1, cx);
        let theme = cx.theme();
        v_flex()
            .flex_1()
            .h_full()
            .min_w_0()
            .gap_3()
            .px_5()
            .py_4()
            .child(
                h_flex()
                    .justify_between()
                    .gap_2()
                    .child(
                        h_flex()
                            .gap_1()
                            .child(previous)
                            .child(
                                div()
                                    .min_w(px(MONTH_TITLE_WIDTH))
                                    .text_center()
                                    .text_lg()
                                    .font_semibold()
                                    .child(self.month.format("%B %Y").to_string()),
                            )
                            .child(next),
                    )
                    .child(legend(cx)),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(h_flex().gap_1().children(WEEKDAYS.map(|weekday| {
                        div()
                            .flex_1()
                            .text_center()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(weekday)
                    })))
                    .children(weeks),
            )
            .child(
                v_flex()
                    .gap_1()
                    .pt_3()
                    .border_t_1()
                    .border_color(theme.border)
                    .text_sm()
                    .child(div().text_base().font_semibold().child(detail.title))
                    .child(div().text_color(theme.muted_foreground).child(detail.when))
                    .children(detail.lines.into_iter().map(|line| div().child(line))),
            )
    }
}

/// Every day of the Monday-first weeks that hold a month's days.
fn month_days(month: NaiveDate) -> impl Iterator<Item = NaiveDate> {
    let first = month.with_day(1).expect("every month has a first day");
    let last = first + Months::new(1) - Days::new(1);
    let start = first - Days::new(first.weekday().num_days_from_monday().into());
    let end = last + Days::new((6 - last.weekday().num_days_from_monday()).into());
    start.iter_days().take_while(move |&day| day <= end)
}

fn area(calendar: &Calendar, date: NaiveDate) -> Option<Area> {
    if calendar.four_day_weekend(date).is_some() {
        return Some(Area::FourDayWeekend);
    }
    (calendar.day(date) == Day::Bridge).then_some(Area::Bridge)
}

fn disc(calendar: &Calendar, date: NaiveDate) -> Option<Disc> {
    match calendar.day(date) {
        Day::BankHoliday => Some(Disc::Filled),
        Day::PredictedBankHoliday => Some(Disc::Dashed),
        Day::Bridge | Day::FourDayWeekend | Day::Ordinary => None,
    }
}

fn area_tint(theme: &ThemeColor, area: Area) -> Hsla {
    match area {
        Area::Bridge => theme.warning.opacity(BRIDGE_TINT),
        Area::FourDayWeekend => theme.red.opacity(FOUR_DAY_WEEKEND_TINT),
    }
}

/// The opaque colour behind a day's numeral, which a bridge day's disc repeats over the hatching
/// to keep its lines off the numeral.
fn cell_background(theme: &ThemeColor, selected: bool, area: Option<Area>) -> Hsla {
    let base = if selected {
        theme.accent
    } else {
        theme.background
    };
    area.map_or(base, |area| base.blend(area_tint(theme, area)))
}

fn bridge_hatch(theme: &ThemeColor) -> Hsla {
    theme.warning.opacity(BRIDGE_HATCH)
}

fn disc_fill(theme: &ThemeColor, overflow: bool) -> Hsla {
    if overflow {
        theme.red.opacity(OVERFLOW_FILL)
    } else {
        theme.red
    }
}

fn numeral_colour(theme: &ThemeColor, disc: Option<Disc>, overflow: bool) -> Hsla {
    match (disc, overflow) {
        (Some(Disc::Filled), false) => theme.background,
        (Some(Disc::Filled), true) | (None | Some(Disc::Dashed), false) => theme.foreground,
        (None | Some(Disc::Dashed), true) => theme.muted_foreground,
    }
}

fn cell(index: usize, date: NaiveDate, look: &Look, cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    let disc = div()
        .size(px(DISC_SIZE))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .text_color(numeral_colour(theme, look.disc, look.overflow))
        .map(|disc| match look.disc {
            None => disc.when(look.area == Some(Area::Bridge), |disc| {
                disc.bg(cell_background(theme, look.selected, look.area))
            }),
            Some(Disc::Dashed) => disc
                .font_semibold()
                .border_1()
                .border_dashed()
                .border_color(theme.red),
            Some(Disc::Filled) => disc.font_semibold().bg(disc_fill(theme, look.overflow)),
        })
        .child(date.day().to_string());
    let ring = div()
        .size(px(RING_SIZE))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .when(look.today, |ring| {
            ring.border_2().border_color(theme.foreground)
        })
        .child(disc);
    let shading = div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(CELL_RADIUS))
        .when(look.area == Some(Area::Bridge), |shading| {
            shading.bg(pattern_slash(
                bridge_hatch(theme),
                BRIDGE_HATCH_WIDTH,
                BRIDGE_HATCH_INTERVAL,
            ))
        })
        .child(ring);
    div()
        .id(("day", index))
        .flex_1()
        .h(px(CELL_HEIGHT))
        .rounded(px(CELL_RADIUS))
        .text_sm()
        .when(look.selected, |cell| cell.bg(theme.accent))
        .child(
            div()
                .size_full()
                .rounded(px(CELL_RADIUS))
                .when_some(look.area, |tint, area| tint.bg(area_tint(theme, area)))
                .child(shading),
        )
}

fn legend(cx: &App) -> Div {
    let theme = cx.theme();
    let swatch = || div().flex_none().size(px(LEGEND_SWATCH_SIZE));
    let entry = |swatch: Div, label: &'static str| h_flex().gap_1().child(swatch).child(label);
    h_flex()
        .gap_3()
        .text_xs()
        .text_color(theme.muted_foreground)
        .child(entry(swatch().rounded_full().bg(theme.red), "Bank holiday"))
        .child(entry(
            swatch()
                .rounded_full()
                .border_1()
                .border_dashed()
                .border_color(theme.red),
            "Predicted",
        ))
        .child(entry(
            swatch()
                .rounded_sm()
                .bg(area_tint(theme, Area::FourDayWeekend)),
            "Four-day weekend",
        ))
        .child(entry(
            swatch()
                .rounded_sm()
                .bg(area_tint(theme, Area::Bridge))
                .child(div().size_full().bg(pattern_slash(
                    bridge_hatch(theme),
                    BRIDGE_HATCH_WIDTH,
                    BRIDGE_HATCH_INTERVAL,
                ))),
            "Bridge",
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    use gpui_kit::Rgba;

    /// WCAG's minimum for text.
    const TEXT_CONTRAST: f32 = 4.5;
    /// WCAG's minimum for graphics, and for overflow days, which repeat days drawn in full in
    /// their own month.
    const GRAPHIC_CONTRAST: f32 = 3.;

    fn date(s: &str) -> NaiveDate {
        s.parse().expect("test dates are valid")
    }

    #[test]
    fn a_month_fills_its_first_and_last_weeks_from_the_months_either_side() {
        let december: Vec<_> = month_days(date("2025-12-17")).collect();
        assert_eq!(
            (december.first(), december.last(), december.len()),
            (Some(&date("2025-12-01")), Some(&date("2026-01-04")), 35),
        );
        let february: Vec<_> = month_days(date("2026-02-01")).collect();
        assert_eq!(
            (february.first(), february.last(), february.len()),
            (Some(&date("2026-01-26")), Some(&date("2026-03-01")), 35),
        );
    }

    fn luminance(colour: Hsla) -> f32 {
        let Rgba { r, g, b, .. } = colour.into();
        let linear = |channel: f32| {
            if channel <= 0.040_45 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
    }

    fn contrast(a: Hsla, b: Hsla) -> f32 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn every_mark_meets_wcag_contrast_in_light_and_dark() {
        let mut failures = Vec::new();
        for (appearance, theme) in [("light", ThemeColor::light()), ("dark", ThemeColor::dark())] {
            let mut check = |what: String, a: Hsla, b: Hsla, minimum: f32| {
                let ratio = contrast(a, b);
                if ratio < minimum {
                    failures.push(format!(
                        "{appearance}: {what} is {ratio:.2}, under {minimum}"
                    ));
                }
            };
            for selected in [false, true] {
                // A bridge books working days, so it never holds a bank holiday's disc.
                for (area, discs) in [
                    (None, &[None, Some(Disc::Dashed), Some(Disc::Filled)][..]),
                    (
                        Some(Area::FourDayWeekend),
                        &[None, Some(Disc::Dashed), Some(Disc::Filled)],
                    ),
                    (Some(Area::Bridge), &[None]),
                ] {
                    let on = format!("{area:?} cell{}", if selected { ", selected" } else { "" });
                    let behind = cell_background(&theme, selected, area);
                    let mut ring_behinds = vec![behind];
                    if area == Some(Area::Bridge) {
                        ring_behinds.push(behind.blend(bridge_hatch(&theme)));
                    }
                    for ring_behind in ring_behinds {
                        check(
                            format!("today's ring on {on}"),
                            theme.foreground,
                            ring_behind,
                            GRAPHIC_CONTRAST,
                        );
                    }
                    for overflow in [false, true] {
                        let text_minimum = if overflow {
                            GRAPHIC_CONTRAST
                        } else {
                            TEXT_CONTRAST
                        };
                        for &disc in discs {
                            let what = format!("{disc:?} disc, overflow {overflow}, on {on}");
                            let numeral_behind = match disc {
                                None => behind,
                                Some(Disc::Dashed) => {
                                    check(
                                        format!("outline of {what}"),
                                        theme.red,
                                        behind,
                                        GRAPHIC_CONTRAST,
                                    );
                                    behind
                                }
                                Some(Disc::Filled) => {
                                    let fill = behind.blend(disc_fill(&theme, overflow));
                                    check(
                                        format!("fill of {what}"),
                                        fill,
                                        behind,
                                        GRAPHIC_CONTRAST,
                                    );
                                    fill
                                }
                            };
                            check(
                                format!("numeral of {what}"),
                                numeral_colour(&theme, disc, overflow),
                                numeral_behind,
                                text_minimum,
                            );
                        }
                    }
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
