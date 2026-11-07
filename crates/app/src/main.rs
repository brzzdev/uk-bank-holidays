use std::process;

use gpui_kit::{
    AppContext, Context, IntoElement, QuitMode, Render, Window, WindowOptions, application, assets,
    div, init, open_window,
};

struct BankHolidays;

impl Render for BankHolidays {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn main() {
    application()
        .with_assets(assets::Assets)
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(|cx| {
            init(cx);
            if let Err(error) = open_window(WindowOptions::default(), cx, |_, cx| {
                cx.new(|_| BankHolidays)
            }) {
                eprintln!("bank-holidays-app: couldn't open a window: {error}");
                process::exit(1);
            }
        });
}
