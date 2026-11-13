use bank_holidays::Calendar;
use chrono::{Datelike as _, Days, Months, NaiveDate, Weekday};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _, StyledExt as _, ThemeColor, h_flex,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Background, Context, Div, Hsla, InteractiveElement as _, IntoElement, ParentElement,
    Stateful, StatefulInteractiveElement as _, Styled, div, pattern_slash, px,
};

use crate::detail::detail;
use crate::selection::Move;
use crate::{BankHolidays, Today, divider, first_of_month};

/// The bridge's hatch lines, as an opacity of `warning`.
const BRIDGE_HATCH: f32 = 0.35;
const BRIDGE_HATCH_INTERVAL: f32 = 4.;
const BRIDGE_HATCH_WIDTH: f32 = 1.5;
/// The amber behind a bridge's days, as an opacity of `warning`.
const BRIDGE_TINT: f32 = 0.12;
/// The red behind a four-day weekend's days, as an opacity of `red`.
const FOUR_DAY_WEEKEND_TINT: f32 = 0.12;
/// A dimmed overflow day's bank holiday disc, as an opacity of `red`.
const OVERFLOW_DISC: f32 = 0.8;

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
    pub(crate) fn month_pane(
        &self,
        calendar: &Calendar,
        today: NaiveDate,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let month = first_of_month(self.selection.date);
        let days: Vec<_> = month_days(month).collect();
        let weeks: Vec<_> = days
            .chunks(WEEKDAYS.len())
            .enumerate()
            .map(|(week, days)| {
                h_flex()
                    .gap_1()
                    .children(days.iter().enumerate().map(|(weekday, &date)| {
                        let look = Look {
                            area: area(calendar, date),
                            disc: disc(calendar, date),
                            overflow: first_of_month(date) != month,
                            selected: date == self.selection.date,
                            today: date == today,
                        };
                        let selectable = self.selection.moved(Move::To(date), calendar).is_some();
                        cell(week * WEEKDAYS.len() + weekday, date, &look, cx)
                            .when(selectable, |cell| {
                                cell.on_click(Self::select_on_click(date, cx))
                            })
                    }))
            })
            .collect();
        let detail = detail(calendar, self.selection.date, today);
        let previous = self.step_button(
            "previous-month",
            IconName::ChevronLeft,
            Move::Months(-1),
            calendar,
            cx,
        );
        let next = self.step_button(
            "next-month",
            IconName::ChevronRight,
            Move::Months(1),
            calendar,
            cx,
        );
        let today_button = Button::new("today")
            .ghost()
            .small()
            .tab_stop(false)
            .label("Today")
            .disabled(!self.can_go_today(calendar, today))
            .on_click(|_, window, cx| window.dispatch_action(Box::new(Today), cx));
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
                    .flex_wrap()
                    .justify_between()
                    .gap_2()
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                div()
                                    .min_w(px(MONTH_TITLE_WIDTH))
                                    .text_lg()
                                    .font_semibold()
                                    .child(month.format("%B %Y").to_string()),
                            )
                            .child(previous)
                            .child(today_button)
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
                    .border_color(divider(theme))
                    .text_sm()
                    .child(div().text_base().font_semibold().child(detail.title))
                    .child(div().text_color(theme.muted_foreground).child(detail.when))
                    .children(detail.lines.into_iter().map(|line| div().child(line))),
            )
    }
}

/// Every day of the Monday-first weeks that hold the month starting on `first`.
fn month_days(first: NaiveDate) -> impl Iterator<Item = NaiveDate> {
    let last = first + Months::new(1) - Days::new(1);
    let start = first.week(Weekday::Mon).first_day();
    let end = last.week(Weekday::Mon).last_day();
    start.iter_days().take_while(move |&day| day <= end)
}

fn area(calendar: &Calendar, date: NaiveDate) -> Option<Area> {
    if calendar.four_day_weekend(date).is_some() {
        return Some(Area::FourDayWeekend);
    }
    calendar.bridge_booked_on(date).map(|_| Area::Bridge)
}

fn disc(calendar: &Calendar, date: NaiveDate) -> Option<Disc> {
    calendar.holiday(date).map(|holiday| {
        if holiday.predicted {
            Disc::Dashed
        } else {
            Disc::Filled
        }
    })
}

fn area_tint(theme: &ThemeColor, area: Area) -> Hsla {
    match area {
        Area::Bridge => theme.warning.opacity(BRIDGE_TINT),
        Area::FourDayWeekend => theme.red.opacity(FOUR_DAY_WEEKEND_TINT),
    }
}

/// The opaque colour of a day's cell, under any hatching.
fn cell_background(theme: &ThemeColor, selected: bool, area: Option<Area>) -> Hsla {
    let base = if selected {
        theme.accent
    } else {
        theme.background
    };
    area.map_or(base, |area| base.blend(area_tint(theme, area)))
}

/// The edge of a shape too faint to tell from the window alone.
fn outline(theme: &ThemeColor) -> Hsla {
    theme.muted_foreground
}

/// The edge of a four-day weekend's day, whose tint is too faint to tell from the window alone.
fn four_day_weekend_edge(theme: &ThemeColor) -> Hsla {
    theme.red
}

fn bridge_hatch(theme: &ThemeColor) -> Hsla {
    theme.warning.opacity(BRIDGE_HATCH)
}

fn bridge_pattern(theme: &ThemeColor) -> Background {
    pattern_slash(
        bridge_hatch(theme),
        BRIDGE_HATCH_WIDTH,
        BRIDGE_HATCH_INTERVAL,
    )
}

fn disc_colour(theme: &ThemeColor, overflow: bool) -> Hsla {
    if overflow {
        theme.red.opacity(OVERFLOW_DISC)
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
            // An opaque disc keeps the hatch lines off the numeral.
            None => disc.when(look.area == Some(Area::Bridge), |disc| {
                disc.bg(cell_background(theme, look.selected, look.area))
            }),
            Some(Disc::Dashed) => disc
                .font_semibold()
                .border_1()
                .border_dashed()
                .border_color(disc_colour(theme, look.overflow)),
            Some(Disc::Filled) => disc.font_semibold().bg(disc_colour(theme, look.overflow)),
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
        .map(|shading| match look.area {
            Some(Area::Bridge) => shading.bg(bridge_pattern(theme)),
            // A selected day's outline is its edge: a second border would crowd today's ring.
            Some(Area::FourDayWeekend) if !look.selected => shading
                .border_1()
                .border_color(four_day_weekend_edge(theme)),
            Some(Area::FourDayWeekend) | None => shading,
        })
        .child(ring);
    div()
        .id(("day", index))
        .relative()
        .flex_1()
        .h(px(CELL_HEIGHT))
        .rounded(px(CELL_RADIUS))
        .text_sm()
        .bg(cell_background(theme, look.selected, look.area))
        .child(shading)
        .when(look.selected, |cell| {
            // Out of the flow, so the outline never widens the cell at its week's expense.
            cell.child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(CELL_RADIUS))
                    .border_2()
                    .border_color(outline(theme)),
            )
        })
}

fn legend(cx: &App) -> Div {
    let theme = cx.theme();
    let swatch = || div().flex_none().size(px(LEGEND_SWATCH_SIZE));
    let shading_swatch = |area| {
        let edge = match area {
            Area::Bridge => outline(theme),
            Area::FourDayWeekend => four_day_weekend_edge(theme),
        };
        swatch()
            .rounded_sm()
            .border_1()
            .border_color(edge)
            .bg(area_tint(theme, area))
    };
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
            shading_swatch(Area::FourDayWeekend),
            "Four-day weekend",
        ))
        .child(entry(
            shading_swatch(Area::Bridge).child(div().size_full().bg(bridge_pattern(theme))),
            "Bridge",
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    use gpui_kit::Rgba;

    /// A divider separates panes without carrying meaning, so it needs less than WCAG's 3:1.
    const DIVIDER_CONTRAST: f32 = 1.5;
    /// WCAG's minimum for graphics.
    const GRAPHIC_CONTRAST: f32 = 3.;
    /// WCAG's minimum for text.
    const TEXT_CONTRAST: f32 = 4.5;

    fn date(s: &str) -> NaiveDate {
        s.parse().expect("test dates are valid")
    }

    #[test]
    fn a_month_fills_its_first_and_last_weeks_from_the_months_either_side() {
        let december: Vec<_> = month_days(date("2025-12-01")).collect();
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
    fn every_edge_and_divider_stands_out_from_the_surface_behind_it_in_light_and_dark() {
        let mut failures = Vec::new();
        for (appearance, theme) in [("light", ThemeColor::light()), ("dark", ThemeColor::dark())] {
            let edges = [
                ("four-day weekend edge", four_day_weekend_edge(&theme)),
                ("outline", outline(&theme)),
            ]
            .map(|(mark, colour)| (mark, colour, "window", theme.background, GRAPHIC_CONTRAST));
            let dividers = [
                ("sidebar", theme.sidebar),
                ("title bar", theme.title_bar),
                ("window", theme.background),
            ]
            .map(|(surface, behind)| {
                (
                    "divider",
                    divider(&theme),
                    surface,
                    behind,
                    DIVIDER_CONTRAST,
                )
            });
            for (mark, colour, surface, behind, minimum) in edges.into_iter().chain(dividers) {
                let ratio = contrast(colour, behind);
                if ratio < minimum {
                    failures.push(format!(
                        "{appearance}: {mark} on the {surface} is {ratio:.2}, under {minimum}"
                    ));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// The four-day weekend tint and the bridge hatching are shading the legend names, so they're
    /// checked as backgrounds for the marks on them, not against a plain cell.
    #[test]
    fn every_mark_meets_wcag_contrast_in_light_and_dark() {
        let looks = [false, true].into_iter().flat_map(|selected| {
            [None, Some(Area::Bridge), Some(Area::FourDayWeekend)]
                .into_iter()
                .flat_map(move |area| {
                    [None, Some(Disc::Dashed), Some(Disc::Filled)]
                        .into_iter()
                        .flat_map(move |disc| {
                            [false, true].map(move |overflow| (selected, area, disc, overflow))
                        })
                })
        });
        let mut failures = Vec::new();
        for (appearance, theme) in [("light", ThemeColor::light()), ("dark", ThemeColor::dark())] {
            for (selected, area, disc, overflow) in looks.clone() {
                // A bridge books working days, so it never holds a bank holiday's disc.
                if area == Some(Area::Bridge) && disc.is_some() {
                    continue;
                }
                let behind = cell_background(&theme, selected, area);
                let fill = behind.blend(disc_colour(&theme, overflow));
                let numeral_behind = if disc == Some(Disc::Filled) {
                    fill
                } else {
                    behind
                };
                // Overflow days repeat days drawn in full in their own month.
                let text_minimum = if overflow {
                    GRAPHIC_CONTRAST
                } else {
                    TEXT_CONTRAST
                };
                let mut checks = vec![
                    ("today's ring", theme.foreground, behind, GRAPHIC_CONTRAST),
                    (
                        "numeral",
                        numeral_colour(&theme, disc, overflow),
                        numeral_behind,
                        text_minimum,
                    ),
                ];
                if selected {
                    checks.push((
                        "selection outline",
                        outline(&theme),
                        behind,
                        GRAPHIC_CONTRAST,
                    ));
                }
                match (area, disc) {
                    (Some(Area::Bridge), _) => checks.push((
                        "today's ring on hatching",
                        theme.foreground,
                        behind.blend(bridge_hatch(&theme)),
                        GRAPHIC_CONTRAST,
                    )),
                    (_, Some(Disc::Dashed)) => {
                        checks.push(("outline", fill, behind, GRAPHIC_CONTRAST));
                    }
                    (_, Some(Disc::Filled)) => {
                        checks.push(("fill", fill, behind, GRAPHIC_CONTRAST));
                    }
                    (_, None) => {}
                }
                for (mark, colour, behind, minimum) in checks {
                    let ratio = contrast(colour, behind);
                    if ratio < minimum {
                        failures.push(format!(
                            "{appearance}: {mark} of {disc:?} disc on {area:?} cell, selected \
                             {selected}, overflow {overflow}, is {ratio:.2}, under {minimum}"
                        ));
                    }
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
