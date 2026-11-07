use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    AppContext, Context, IntoElement, Render, Styled, Window, WindowOptions, application, assets,
    div, init, open_window,
};

struct BankHolidays;

impl Render for BankHolidays {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(cx.theme().background)
    }
}

fn main() {
    application().with_assets(assets::Assets).run(|cx| {
        init(cx);
        if let Err(error) = open_window(WindowOptions::default(), cx, |_, cx| {
            cx.new(|_| BankHolidays)
        }) {
            eprintln!("bank-holidays-app: couldn't open a window: {error}");
            cx.quit();
        }
    });
}
