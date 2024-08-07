use dex_io::*;
use gear_lib::tx_manager::Stepper;
use gstd::{
    errors::Result,
    msg::{self, CodecMessageFuture},
    prelude::*,
    ActorId,
};
use sharded_fungible_token_io::{FTokenAction, FTokenEvent, LogicAction};
use fungible_token_io::{FTAction,FTEvent};

pub fn send<T: Decode>(to: ActorId, payload: impl Encode) -> Result<CodecMessageFuture<T>> {
    Ok(msg::send_for_reply_as(to, payload, 0, 0)?)
}

pub async fn transfer_tokens(
    token: ActorId,
    sender: ActorId,
    recipient: ActorId,
    amount: u128,
) -> Result<(), Error> {
    // let payload = FTokenAction::Message {
    //     transaction_id: stepper.step()?,
    //     payload: LogicAction::Transfer {
    //         sender,
    //         recipient,
    //         amount,
    //     },
    // };

    let payload = FTAction::Transfer {
        from: sender,
        to: recipient,
        amount,
    };

    match send(token, payload)?.await? {
        FTEvent::Transfer { from, to, amount } => Ok(()),
        // FTokenEvent::Err => Err(Error::TransferFailed),
        _ => unreachable!("received an unexpected `FTokenEvent` variant"),
    }
}

pub async fn balance_of(token: ActorId, actor: ActorId) -> Result<u128> {
    if let FTokenEvent::Balance(balance) = send(token, FTokenAction::GetBalance(actor))?.await? {
        Ok(balance)
    } else {
        unreachable!("received an unexpected `FTokenEvent` variant");
    }
}

pub fn sort_tokens(token_a: ActorId, token_b: ActorId) -> (ActorId, ActorId) {
    let token_pair = if token_b > token_a {
        (token_b, token_a)
    } else {
        (token_a, token_b)
    };
    token_pair
}
