use amethyst::ecs::{Join, System, WriteExpect, WriteStorage};

use std::iter::FromIterator;

use gv_core::{
    ecs::{
        resources::world::{
            ClientWorldUpdates, ImmediatePlayerActionsUpdates, PlayerLookActionUpdates,
        },
        system_data::time::GameTimeService,
    },
    net::{client_message::ClientMessagePayload, NetConnection, INTERPOLATION_FRAME_DELAY},
};
use gv_game::{ecs::system_data::GameStateHelper, utils::net::send_message_reliable};

const BROADCAST_FRAME_INTERVAL: u64 = 5;

#[derive(Default)]
pub struct GameUpdatesBroadcastingSystem {
    last_broadcasted_frame: u64,
}

impl<'s> System<'s> for GameUpdatesBroadcastingSystem {
    type SystemData = (
        GameTimeService<'s>,
        GameStateHelper<'s>,
        WriteExpect<'s, ClientWorldUpdates>,
        WriteStorage<'s, NetConnection>,
    );

    fn run(
        &mut self,
        (
            game_time_service,
            game_state_helper,
            mut client_world_updates,
            mut net_connections,
        ): Self::SystemData,
    ) {
        if !game_state_helper.multiplayer_is_running() {
            return;
        }

        let net_connection = (&mut net_connections)
            .join()
            .next()
            .expect("Expected a server connection");

        if !client_world_updates.walk_action_updates.is_empty() {
            send_message_reliable(
                net_connection,
                &ClientMessagePayload::WalkActions(ImmediatePlayerActionsUpdates {
                    frame_number: game_time_service.game_frame_number() + INTERPOLATION_FRAME_DELAY,
                    updates: client_world_updates.walk_action_updates.clone(),
                }),
            );
            client_world_updates.walk_action_updates.clear();
        }

        if !client_world_updates.cast_action_updates.is_empty() {
            send_message_reliable(
                net_connection,
                &ClientMessagePayload::CastActions(ImmediatePlayerActionsUpdates {
                    frame_number: game_time_service.game_frame_number() + INTERPOLATION_FRAME_DELAY,
                    updates: client_world_updates.cast_action_updates.clone(),
                }),
            );
            client_world_updates.cast_action_updates.clear();
        }

        let is_time_to_broadcast = game_time_service
            .game_frame_number()
            .wrapping_sub(self.last_broadcasted_frame)
            > BROADCAST_FRAME_INTERVAL;
        if !is_time_to_broadcast {
            return;
        }
        self.last_broadcasted_frame = game_time_service.game_frame_number();

        send_message_reliable(
            net_connection,
            &ClientMessagePayload::LookActions(PlayerLookActionUpdates {
                updates: Vec::from_iter(client_world_updates.look_actions_updates.drain(..).map(
                    |(frame_number, update)| (frame_number + INTERPOLATION_FRAME_DELAY, update),
                )),
            }),
        );
    }
}
