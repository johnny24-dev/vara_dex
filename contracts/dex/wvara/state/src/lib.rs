#![no_std]

use gstd::{prelude::*, ActorId};

#[gmeta::metawasm]
pub mod metafns {

    pub type State = dex_wvara_io::WVaraState;

    pub fn name(state: State) -> String {
        state.name
    }

    pub fn symbol(state: State) -> String {
        state.symbol
    }

    pub fn decimals(state: State) -> u64 {
        state.decimals
    }

    pub fn balance_of(state: State, actor: ActorId) -> u128 {
        state.balance_of(actor)
    }

    pub fn allowance(state: State, owner: ActorId, spender: ActorId) -> u128 {
        state.allowance(owner, spender)
    }

    pub fn total_supply(state: State) -> u128 {
        state.total_supply
    }
}
