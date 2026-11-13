use bank_holidays::Division;
use gpui_kit::{App, Menu, MenuItem, SystemMenuType, actions};
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::NSApplication;

use crate::{
    DIVISIONS, NextBankHoliday, NextMonth, NextYear, PreviousBankHoliday, PreviousMonth,
    PreviousYear, ShowEnglandAndWales, ShowNorthernIreland, ShowScotland, Today,
};

actions!(
    bank_holidays,
    [About, Close, Hide, HideOthers, Minimise, Quit, ShowAll]
);

const APP_NAME: &str = "Bank Holidays";

/// Go's items are greyed by GPUI wherever the window hasn't registered them.
pub(crate) fn menus() -> [Menu; 4] {
    [
        Menu::new(APP_NAME).items([
            MenuItem::action(format!("About {APP_NAME}"), About),
            MenuItem::separator(),
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action(format!("Hide {APP_NAME}"), Hide),
            MenuItem::action("Hide Others", HideOthers),
            MenuItem::action("Show All", ShowAll),
            MenuItem::separator(),
            MenuItem::action(format!("Quit {APP_NAME}"), Quit),
        ]),
        Menu::new("File").items([MenuItem::action("Close", Close)]),
        Menu::new("Window").items([MenuItem::action("Minimise", Minimise)]),
        Menu::new("Go").items(
            [
                MenuItem::action("Today", Today),
                MenuItem::separator(),
                MenuItem::action("Previous Month", PreviousMonth),
                MenuItem::action("Next Month", NextMonth),
                MenuItem::separator(),
                MenuItem::action("Previous Year", PreviousYear),
                MenuItem::action("Next Year", NextYear),
                MenuItem::separator(),
                MenuItem::action("Previous Bank Holiday", PreviousBankHoliday),
                MenuItem::action("Next Bank Holiday", NextBankHoliday),
                MenuItem::separator(),
            ]
            .into_iter()
            .chain(DIVISIONS.map(|(division, label)| match division {
                Division::EnglandAndWales => MenuItem::action(label, ShowEnglandAndWales),
                Division::NorthernIreland => MenuItem::action(label, ShowNorthernIreland),
                Division::Scotland => MenuItem::action(label, ShowScotland),
            })),
        ),
    ]
}

/// Close and Minimise act on `NSApp`'s key window, so they reach the About panel too.
pub(crate) fn on_app_commands(cx: &mut App) {
    cx.on_action(|_: &About, _| shared_application().orderFrontStandardAboutPanel(None));
    cx.on_action(|_: &Close, _| {
        if let Some(window) = shared_application().keyWindow() {
            window.performClose(None);
        }
    });
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &Minimise, _| {
        if let Some(window) = shared_application().keyWindow() {
            window.performMiniaturize(None);
        }
    });
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
}

fn shared_application() -> Retained<NSApplication> {
    let main_thread = MainThreadMarker::new().expect("GPUI dispatches actions on the main thread");
    NSApplication::sharedApplication(main_thread)
}
