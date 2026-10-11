use bevy::{asset::AssetMetaCheck, prelude::*};
use hexfall::GamePlugin;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                // Nearest-neighbour sampling keeps pixel art crisp.
                .set(ImagePlugin::default_nearest())
                // The assets ship without `.meta` files; skip the failing request per asset.
                .set(AssetPlugin {
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Hexfall".into(),
                        resolution: (1280, 720).into(),
                        // Web: fill the browser window and keep keys like Space from scrolling the page.
                        fit_canvas_to_parent: true,
                        prevent_default_event_handling: true,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(GamePlugin)
        .run();
}
