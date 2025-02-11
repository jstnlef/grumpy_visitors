#![feature(drain_filter)]
#![allow(clippy::type_complexity, clippy::too_many_arguments)]
pub mod ecs;
pub mod states;
pub mod utils;

use amethyst::{
    error::Error,
    prelude::{GameDataBuilder, World, WorldExt},
};

use gv_core::{
    actions::monster_spawn::SpawnActions,
    ecs::{
        components::damage_history::DamageHistory,
        resources::{
            net::{
                ActionUpdateIdProvider, CastActionsToExecute, EntityNetMetadataStorage,
                MultiplayerGameState,
            },
            world::{FramedUpdates, PlayerActionUpdates, WorldStates},
        },
    },
};

use crate::ecs::{
    resources::ConnectionEvents,
    systems::{missile::MissileDyingSystem, monster::*, *},
};

pub fn build_game_logic_systems<'a, 'b>(
    game_data_builder: GameDataBuilder<'a, 'b>,
    world: &mut World,
    is_server: bool,
) -> Result<GameDataBuilder<'a, 'b>, Error> {
    world.insert(FramedUpdates::<PlayerActionUpdates>::default());
    world.insert(FramedUpdates::<SpawnActions>::default());
    world.insert(WorldStates::default());
    world.insert(ConnectionEvents(Vec::new()));
    world.insert(MultiplayerGameState::new());
    world.insert(EntityNetMetadataStorage::new());
    world.insert(ActionUpdateIdProvider::default());
    world.insert(CastActionsToExecute::default());

    world.register::<DamageHistory>();

    let game_data_builder = game_data_builder
        .with(PauseSystem, "pause_system", &["game_network_system"])
        .with(LevelSystem::default(), "level_system", &["pause_system"])
        .with(MonsterSpawnerSystem, "spawner_system", &["level_system"])
        .with(
            ActionSystem,
            "action_system",
            &dependencies_with_optional(&["spawner_system"], !is_server, &["input_system"]),
        )
        .with(
            MonsterDyingSystem,
            "monster_dying_system",
            &["action_system"],
        )
        .with(
            MissileDyingSystem,
            "missile_dying_system",
            &["action_system"],
        )
        .with(
            WorldPositionTransformSystem,
            "world_position_transform_system",
            &["action_system"],
        )
        .with(
            StateSwitcherSystem,
            "state_switcher_system",
            &dependencies_with_optional(
                &["monster_dying_system", "missile_dying_system"],
                !is_server,
                &["menu_system"],
            ),
        );
    Ok(game_data_builder)
}

fn optional_dependencies(dependencies: &[&'static str], condition: bool) -> Vec<&'static str> {
    if condition {
        dependencies.to_vec()
    } else {
        Vec::new()
    }
}

fn dependencies_with_optional(
    mandatory: &[&'static str],
    condition: bool,
    optional: &[&'static str],
) -> Vec<&'static str> {
    let mut dependencies = mandatory.to_vec();
    dependencies.append(&mut optional_dependencies(optional, condition));
    dependencies
}
