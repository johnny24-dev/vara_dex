#![no_std]
use dex_test_transfer_io::*;
use dex_wvara_io::{Action as WVaraAction, Error as WVaraError, Event as WVaraEvent};
use fungible_token_io::{FTAction, FTEvent};
use gstd::{
    collections::HashMap,
    errors::Result,
    exec::{self, program_id},
    msg::{self, CodecMessageFuture},
    prelude::*,
    prog::ProgramGenerator,
    ActorId, CodeId, MessageId,
};

#[gstd::async_main]
async fn main() {
    reply(process_handle().await).expect("failed to encode or reply `handle()`");
}

pub fn send<T: Decode>(to: ActorId, payload: impl Encode) -> Result<CodecMessageFuture<T>> {
    Ok(msg::send_for_reply_as(to, payload, 0, 0)?)
}

pub async fn transfer_tokens(
    token: ActorId,
    sender: ActorId,
    recipient: ActorId,
    amount: u128,
) -> Result<(), Error> {
    let payload = FTAction::Transfer {
        from: sender,
        to: recipient,
        amount,
    };
    let send_result:Result<FTEvent,E> = send(token, payload)?.await?;
    let Ok(FTEvent::Transfer { from, to, amount }) = send_result else {
        return Err(Error::TransferFailed);
    };
    return Ok(());
}

pub async fn transfer_wvara(wvara: ActorId, recipient: ActorId, amount: u128) -> Result<(), Error> {
    let send_res: Result<WVaraEvent, WVaraError> = send(
        wvara,
        WVaraAction::Transfer {
            from: program_id(),
            to: recipient,
            amount,
        },
    )?
    .await?;

    let Ok(WVaraEvent::Transfer { from, to, amount }) = send_res else {
        return Err(Error::TransferFailed);
    };
    return Ok(());
}

async fn process_handle() -> Result<Event, Error> {
    let action: Action = msg::load().expect("failed to decode `Action`");

    match action {
        Action::Transfer { token, to, amount } => {
            let call_res = transfer_tokens(token, program_id(), to, amount).await;
            if call_res.is_err() {
                return Err(Error::TransferFailedAction);
            };
            return Ok(Event::Transfer {
                from: program_id(),
                to,
                amount,
            });
        },
        Action::TransferWVara { wvara, to, amount } => {
            let call_res = transfer_wvara(wvara, to, amount).await;
            if call_res.is_err() {
                return Err(Error::TransferWVaraFailedAction);
            };
            return Ok(Event::Transfer {
                from: program_id(),
                to,
                amount,
            });
        }
    }
}

fn reply(payload: impl Encode) -> Result<MessageId> {
    Ok(msg::reply(payload, 0)?)
}
