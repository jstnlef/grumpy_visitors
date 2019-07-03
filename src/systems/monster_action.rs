use amethyst::{
    core::Time,
    ecs::{Entities, Entity, Join, ReadExpect, ReadStorage, System, WriteStorage},
};

use std::time::Duration;

use crate::models::mob_actions::MobAttackType;
use crate::{
    components::{DamageHistory, Monster, Player, WorldPosition},
    data_resources::{GameScene, MonsterDefinitions},
    models::{
        common::DamageHistoryEntry,
        mob_actions::{MobAction, MobActionType, MobAttackAction},
    },
    utils::world::random_scene_position,
    Vector2,
};

const IDLE_TIME_SEC: f32 = 0.5;
const ATTACK_COOLDOWN_MS: f32 = 1.0;

pub struct MonsterActionSystem;

impl<'s> System<'s> for MonsterActionSystem {
    type SystemData = (
        Entities<'s>,
        ReadExpect<'s, Time>,
        ReadExpect<'s, MonsterDefinitions>,
        ReadExpect<'s, GameScene>,
        ReadStorage<'s, Player>,
        ReadStorage<'s, WorldPosition>,
        WriteStorage<'s, DamageHistory>,
        WriteStorage<'s, Monster>,
    );

    fn run(
        &mut self,
        (
            entities,
            time,
            monster_definitions,
            game_scene,
            players,
            world_positions,
            mut damage_histories,
            mut monsters,
        ): Self::SystemData,
    ) {
        for (mut monster, monster_position) in (&mut monsters, &world_positions).join() {
            let monster_definition = monster_definitions
                .0
                .get(&monster.name)
                .expect("Expected a monster definition");

            let new_action_type = match monster.action.action_type {
                MobActionType::Idle => {
                    if let Some((entity, _player_position)) = find_player_in_radius(
                        (&entities, &players, &world_positions).join(),
                        **monster_position,
                        200.0,
                    ) {
                        Some(MobActionType::Chase(entity))
                    } else {
                        let time_being_idle = time.absolute_time() - monster.action.started_at;
                        let max_idle_duration =
                            Duration::from_millis((IDLE_TIME_SEC as f32 * 1000.0).round() as u64);
                        if time_being_idle > max_idle_duration {
                            Some(MobActionType::Move(random_scene_position(&*game_scene)))
                        } else {
                            None
                        }
                    }
                }
                MobActionType::Move(destination) => {
                    if let Some((entity, _player_position)) = find_player_in_radius(
                        (&entities, &players, &world_positions).join(),
                        **monster_position,
                        200.0,
                    ) {
                        Some(MobActionType::Chase(entity))
                    } else if (**monster_position - destination).norm_squared() < 0.01 {
                        Some(MobActionType::Idle)
                    } else {
                        None
                    }
                }
                MobActionType::Chase(_) => {
                    if let Some((target, _player_position)) = find_player_in_radius(
                        (&entities, &players, &world_positions).join(),
                        **monster_position,
                        monster.radius,
                    ) {
                        let damage_history = damage_histories
                            .get_mut(target)
                            .expect("Expected player's DamageHistory");
                        damage_history.add_entry(
                            time.absolute_time(),
                            DamageHistoryEntry {
                                damage: monster.attack_damage,
                            },
                        );
                        Some(MobActionType::Attack(MobAttackAction {
                            target,
                            attack_type: monster_definition.attack_type,
                        }))
                    } else {
                        None
                    }
                }
                MobActionType::Attack(ref attack_action) => {
                    let is_cooling_down = time.absolute_time() - monster.action.started_at
                        < Duration::from_millis((ATTACK_COOLDOWN_MS as f32 * 1000.0).round() as u64);
                    let player_in_radius = find_player_in_radius(
                        (&entities, &players, &world_positions).join(),
                        **monster_position,
                        monster.radius,
                    );
                    match (attack_action.attack_type, player_in_radius) {
                        // TODO: implement cooling down for other attacks as well.
                        (MobAttackType::SlowMelee, _) if is_cooling_down => None,
                        (_, Some((target, _player_position))) => {
                            Some(MobActionType::Attack(MobAttackAction {
                                target,
                                attack_type: monster_definition.attack_type,
                            }))
                        }
                        (_, None) => Some(MobActionType::Idle),
                    }
                }
            };

            let new_destination = if let Some(ref new_action_type) = new_action_type {
                match new_action_type {
                    MobActionType::Move(position) => Some(*position),
                    MobActionType::Chase(entity) => Some(**world_positions.get(*entity).unwrap()),
                    MobActionType::Attack(MobAttackAction {
                        target,
                        attack_type,
                    }) => match attack_type {
                        MobAttackType::Melee => Some(**world_positions.get(*target).unwrap()),
                        _ => Some(**monster_position),
                    },
                    _ => None,
                }
            } else {
                match monster.action.action_type {
                    MobActionType::Chase(entity) => Some(**world_positions.get(entity).unwrap()),
                    _ => None,
                }
            };

            if let Some(destination) = new_destination {
                monster.destination = destination;
            }

            if let Some(action_type) = new_action_type {
                monster.action = MobAction {
                    started_at: time.absolute_time(),
                    action_type,
                }
            }
        }
    }
}

fn find_player_in_radius<'a>(
    mut players: impl Iterator<Item = (Entity, &'a Player, &'a WorldPosition)>,
    position: Vector2,
    radius: f32,
) -> Option<(Entity, &'a WorldPosition)> {
    let radius_squared = radius * radius;
    players
        .find(|(_, player, player_position)| {
            let player_radius_squared = player.radius * player.radius;
            (position - ***player_position).norm_squared() < radius_squared + player_radius_squared
        })
        .map(|(entity, _, player_position)| (entity, player_position))
}
