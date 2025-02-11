pub mod damage_history;
pub mod missile;

use amethyst::{
    ecs::{Component, DenseVecStorage, Entity, ReaderId, VecStorage},
    network::NetEvent,
};
use serde_derive::{Deserialize, Serialize};
use shrinkwraprs::Shrinkwrap;

use std::{collections::VecDeque, time::Instant};

use crate::{
    actions::{
        mob::MobAction,
        player::{PlayerCastAction, PlayerLookAction, PlayerWalkAction},
        Action,
    },
    math::{Vector2, ZeroVector},
    net::{EncodedMessage, NetIdentifier},
};

const PING_PONG_STORAGE_LIMIT: usize = 20;

#[derive(Clone, Debug, Serialize, Deserialize, Shrinkwrap)]
#[shrinkwrap(mutable)]
pub struct WorldPosition {
    #[shrinkwrap(main_field)]
    pub position: Vector2,
}

impl WorldPosition {
    pub fn new(position: Vector2) -> Self {
        Self { position }
    }
}

impl Component for WorldPosition {
    type Storage = VecStorage<Self>;
}

/// On client side this component stores a WorldPosition that a player had
/// `INTERPOLATION_FRAME_DELAY` frames ago.
/// This component isn't used on server side and in single player.
#[derive(Clone, Debug, Serialize, Deserialize, Shrinkwrap)]
#[shrinkwrap(mutable)]
pub struct NetWorldPosition {
    #[shrinkwrap(main_field)]
    pub position: Vector2,
}

impl NetWorldPosition {
    pub fn new(position: Vector2) -> Self {
        Self { position }
    }
}

impl Component for NetWorldPosition {
    type Storage = DenseVecStorage<Self>;
}

impl From<WorldPosition> for NetWorldPosition {
    fn from(world_position: WorldPosition) -> Self {
        NetWorldPosition {
            position: world_position.position,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Player {
    pub health: f32,
    pub velocity: Vector2,
    pub walking_direction: Vector2,
    pub looking_direction: Vector2,
    pub radius: f32,
}

impl Player {
    pub fn new() -> Self {
        Self {
            health: 100.0,
            velocity: Vector2::zero(),
            walking_direction: Vector2::new(0.0, 1.0),
            looking_direction: Vector2::new(0.0, 1.0),
            radius: 20.0,
        }
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Player {
    type Storage = DenseVecStorage<Self>;
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct PlayerActions {
    pub walk_action: PlayerWalkAction,
    pub look_action: PlayerLookAction,
    pub cast_action: Option<PlayerCastAction>,
}

impl Component for PlayerActions {
    type Storage = DenseVecStorage<Self>;
}

/// We write the actions to this component right on input from client, they get processed and
/// inserted to PlayerActions component (and optionally scheduled to be sent to a server)
/// in ActionSystem.
#[derive(Default, Debug, Clone)]
pub struct ClientPlayerActions {
    pub walk_action: PlayerWalkAction,
    pub look_action: PlayerLookAction,
    pub cast_action: Option<PlayerCastAction>,
}

impl Component for ClientPlayerActions {
    type Storage = DenseVecStorage<Self>;
}

/// Stores frame numbers.
#[derive(Clone, Default)]
pub struct PlayerLastCastedSpells {
    pub missile: u64,
}

impl Component for PlayerLastCastedSpells {
    type Storage = DenseVecStorage<Self>;
}

#[derive(Clone, Debug)]
pub struct Monster {
    pub health: f32,
    pub attack_damage: f32,
    pub destination: Vector2,
    pub velocity: Vector2,
    pub action: Action<MobAction<Entity>>,
    pub name: String,
    pub radius: f32,
}

impl Component for Monster {
    type Storage = DenseVecStorage<Self>;
}

#[derive(Clone, Default)]
pub struct Dead {
    pub dead_since_frame: u64,
}

impl Dead {
    pub fn new(dead_since_frame: u64) -> Self {
        Self { dead_since_frame }
    }
}

impl Component for Dead {
    type Storage = VecStorage<Self>;
}

pub struct NetConnectionModel {
    pub id: NetIdentifier,
    pub reader: ReaderId<NetEvent<EncodedMessage>>,
    pub created_at: Instant,
    pub last_acknowledged_update: Option<u64>,
    pub ping_pong_data: PingPongData,
}

impl NetConnectionModel {
    pub fn new(id: NetIdentifier, reader: ReaderId<NetEvent<EncodedMessage>>) -> Self {
        Self {
            id,
            reader,
            created_at: Instant::now(),
            last_acknowledged_update: None,
            ping_pong_data: PingPongData::new(),
        }
    }
}

pub struct PingPongData {
    pub last_pinged_at: Instant,
    pub last_ponged_frame: u64,
    data: VecDeque<PingPong>,
}

impl PingPongData {
    fn new() -> Self {
        Self {
            last_pinged_at: Instant::now(),
            last_ponged_frame: 0,
            data: VecDeque::with_capacity(PING_PONG_STORAGE_LIMIT),
        }
    }

    pub fn add_ping(&mut self, ping_id: NetIdentifier, engine_frame_number: u64) {
        self.last_pinged_at = Instant::now();
        if self.data.len() == PING_PONG_STORAGE_LIMIT {
            self.data.pop_front();
        }
        self.data.push_back(PingPong {
            ping_id,
            sent_ping_frame: engine_frame_number,
            pong: None,
        })
    }

    pub fn add_pong(
        &mut self,
        ping_id: NetIdentifier,
        peer_frame_number: u64,
        engine_frame_number: u64,
        frame_number: u64,
    ) {
        if self.last_ponged_frame < engine_frame_number {
            self.last_ponged_frame = engine_frame_number;
        }

        if let Some(ping_pong) = self
            .data
            .iter_mut()
            .find(|ping_pong| ping_pong.ping_id == ping_id)
        {
            let oneway_latency = engine_frame_number.saturating_sub(ping_pong.sent_ping_frame) / 2;
            let estimated_peer_frame_number = peer_frame_number + oneway_latency;
            ping_pong.pong = Some(Pong {
                received_frame: frame_number,
                estimated_peer_frame_number,
            })
        }
    }

    /// Returns 0 if a level has just started and we have little data, otherwise returns u64::max()
    /// if there're no pongs at all.
    /// This usually evaluates to 0 on client side.
    pub fn average_lagging_behind(&self) -> u64 {
        let (pongs_count, lagging_behind_sum) = self.data.iter().fold(
            (0, 0),
            |(mut pongs_count, mut lagging_behind_sum), ping_pong| {
                if let Some(pong) = &ping_pong.pong {
                    pongs_count += 1;
                    lagging_behind_sum += pong
                        .received_frame
                        .saturating_sub(pong.estimated_peer_frame_number);
                }
                (pongs_count, lagging_behind_sum)
            },
        );
        if pongs_count == 0 {
            if self.data.len() < PING_PONG_STORAGE_LIMIT / 2 {
                0
            } else {
                u64::max_value()
            }
        } else {
            lagging_behind_sum / pongs_count
        }
    }

    pub fn last_stored_game_frame(&self) -> u64 {
        self.data
            .iter()
            .rev()
            .find(|ping_pong| ping_pong.pong.is_some())
            .map(|ping_pong| ping_pong.pong.as_ref().unwrap().estimated_peer_frame_number)
            .unwrap_or(0)
    }
}

struct PingPong {
    ping_id: NetIdentifier,
    sent_ping_frame: u64,
    pong: Option<Pong>,
}

struct Pong {
    received_frame: u64,
    estimated_peer_frame_number: u64,
}

impl Component for NetConnectionModel {
    type Storage = DenseVecStorage<Self>;
}

#[derive(Clone, Copy, Debug)]
pub struct EntityNetMetadata {
    pub id: NetIdentifier,
    pub spawned_frame_number: u64,
}

impl Component for EntityNetMetadata {
    type Storage = VecStorage<Self>;
}
