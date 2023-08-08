use std::collections::HashMap;

use fvm_ipld_blockstore::Blockstore;
use fvm_shared::{
    address::{Address, Protocol},
    ActorID,
};

use fil_actors_runtime::{MessageAccumulator, DEFAULT_CONF, FIRST_NON_SINGLETON_ADDR};

use crate::state::AddressMap;
use crate::State;

pub struct StateSummary {
    pub ids_by_address: HashMap<Address, ActorID>,
    pub next_id: ActorID,
}

// Checks internal invariants of init state.
pub fn check_state_invariants<BS: Blockstore>(
    state: &State,
    store: &BS,
) -> (StateSummary, MessageAccumulator) {
    let acc = MessageAccumulator::default();

    acc.require(!state.network_name.is_empty(), "network name is empty");
    acc.require(
        state.next_id >= FIRST_NON_SINGLETON_ADDR,
        format!("next id {} is too low", state.next_id),
    );

    let mut init_summary = StateSummary { ids_by_address: HashMap::new(), next_id: state.next_id };

    let mut stable_address_by_id = HashMap::<ActorID, Address>::new();
    let mut delegated_address_by_id = HashMap::<ActorID, Address>::new();

    match AddressMap::load(store, &state.address_map, DEFAULT_CONF, "addresses") {
        Ok(address_map) => {
            let ret = address_map.for_each(|key, actor_id| {
                acc.require(key.protocol() != Protocol::ID, format!("key {key} is an ID address"));
                acc.require(
                    actor_id >= &FIRST_NON_SINGLETON_ADDR,
                    format!("unexpected singleton ID value {actor_id}"),
                );

                match key.protocol() {
                    Protocol::ID => {
                        acc.add(format!("key {key} is an ID address"));
                    }
                    Protocol::Delegated => {
                        if let Some(duplicate) = delegated_address_by_id.insert(*actor_id, key) {
                            acc.add(format!(
                                "duplicate mapping to ID {actor_id}: {key} {duplicate}"
                            ));
                        }
                    }
                    _ => {
                        if let Some(duplicate) = stable_address_by_id.insert(*actor_id, key) {
                            acc.add(format!(
                                "duplicate mapping to ID {actor_id}: {key} {duplicate}"
                            ));
                        }
                    }
                }

                init_summary.ids_by_address.insert(key, *actor_id);

                Ok(())
            });

            acc.require_no_error(ret, "error iterating address map");
        }
        Err(e) => acc.add(format!("error loading address map: {e}")),
    }

    (init_summary, acc)
}
