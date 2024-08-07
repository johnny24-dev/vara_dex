#![no_std]

use gstd::{prelude::*, ActorId};

#[gmeta::metawasm]
pub mod metafns {
    pub type State = dex_router_io::State;


    pub fn factory(state: State) -> ActorId {
        state.factory
    }

    pub fn wvara(state: State) -> ActorId {
        state.wvara
    }
}
