use bevy::prelude::*;
use rust_rogue_like::GamePlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Rust Rogue-like".into(),
                resolution: (1280, 720).into(),
                // Web: fill the browser window and keep keys like Space from scrolling the page.
                fit_canvas_to_parent: true,
                prevent_default_event_handling: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(GamePlugin)
        .run();
}
