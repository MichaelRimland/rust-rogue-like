//! In-run heads-up display: experience bar, health bar, level, time and kills.

use bevy::prelude::*;

use super::{ACCENT_COLOR, MUTED_TEXT_COLOR, PANEL_COLOR, TEXT_COLOR, format_time, label};
use crate::{
    AppState,
    logic::skills::Cooldown,
    player::{Health, Player},
    run::{RunStats, Stats, Xp},
    skills::MovementSkills,
};

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), spawn_hud)
            .add_systems(
                Update,
                (update_hud, update_skill_text).run_if(in_state(AppState::InGame)),
            );
    }
}

#[derive(Component)]
struct XpFill;

#[derive(Component)]
struct HealthFill;

#[derive(Component)]
struct HealthText;

#[derive(Component)]
struct InfoText;

#[derive(Component)]
struct SkillText;

fn spawn_hud(mut commands: Commands) {
    // Experience bar across the top edge.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: px(0.0),
            left: px(0.0),
            width: percent(100.0),
            height: px(10.0),
            ..default()
        },
        BackgroundColor(PANEL_COLOR),
        DespawnOnExit(AppState::InGame),
        children![(
            XpFill,
            Node {
                width: percent(0.0),
                height: percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.35, 0.70, 1.0)),
        )],
    ));

    // Health bar in the top-left corner.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: px(20.0),
            left: px(16.0),
            flex_direction: FlexDirection::Column,
            row_gap: px(4.0),
            ..default()
        },
        DespawnOnExit(AppState::InGame),
        children![
            (HealthText, label("HP", 16.0, TEXT_COLOR)),
            (
                Node {
                    width: px(220.0),
                    height: px(14.0),
                    ..default()
                },
                BackgroundColor(PANEL_COLOR),
                children![(
                    HealthFill,
                    Node {
                        width: percent(100.0),
                        height: percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.85, 0.20, 0.25)),
                )],
            ),
            (SkillText, label("", 14.0, MUTED_TEXT_COLOR)),
        ],
    ));

    // Level, timer and kills in the top-center.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: px(18.0),
            width: percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        DespawnOnExit(AppState::InGame),
        children![(InfoText, label("", 22.0, ACCENT_COLOR))],
    ));
}

/// Cooldown readout, e.g. `Dash [Space] ready   Blink [RMB] 2.4s`.
fn update_skill_text(
    stats: Res<Stats>,
    skills: Res<MovementSkills>,
    mut text: Single<&mut Text, With<SkillText>>,
) {
    let readout = |name: &str, key: &str, cooldown: &Cooldown| {
        if cooldown.is_ready() {
            format!("{name} [{key}] ready")
        } else {
            format!("{name} [{key}] {:.1}s", cooldown.remaining())
        }
    };
    // Only skills the player has unlocked are listed.
    let mut lines = Vec::new();
    if stats.dash.level > 0 {
        lines.push(readout("Dash", "Space", &skills.dash));
    }
    if stats.blink.level > 0 {
        lines.push(readout("Blink", "RMB", &skills.blink));
    }
    text.0 = lines.join("\n");
}

fn update_hud(
    xp: Res<Xp>,
    run: Res<RunStats>,
    health: Single<&Health, With<Player>>,
    mut xp_fill: Single<&mut Node, (With<XpFill>, Without<HealthFill>)>,
    mut health_fill: Single<&mut Node, (With<HealthFill>, Without<XpFill>)>,
    mut health_text: Single<&mut Text, (With<HealthText>, Without<InfoText>)>,
    mut info_text: Single<&mut Text, (With<InfoText>, Without<HealthText>)>,
) {
    xp_fill.width = percent(xp.progress() * 100.0);
    health_fill.width = percent(health.fraction() * 100.0);
    health_text.0 = format!("HP {:.0} / {:.0}", health.current.max(0.0), health.max);
    info_text.0 = format!(
        "Lv {}     {}     Kills {}",
        xp.level,
        format_time(run.elapsed),
        run.kills
    );
}
