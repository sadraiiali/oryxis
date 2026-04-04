mod app;
mod theme;

use iced::{window, Size};

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter("oryxis=debug,info")
        .init();

    tracing::info!("Starting Oryxis");

    iced::application(app::Oryxis::boot, app::Oryxis::update, app::Oryxis::view)
        .title(app::Oryxis::title)
        .theme(app::Oryxis::theme)
        .subscription(app::Oryxis::subscription)
        .window(window::Settings {
            size: Size::new(1200.0, 750.0),
            min_size: Some(Size::new(800.0, 500.0)),
            ..Default::default()
        })
        .antialiasing(true)
        .run()
}
