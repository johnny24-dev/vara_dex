#![no_std]

use dex_wvara_io::*;
use gstd::{
    collections::HashMap, errors::Result, exec, msg, prelude::*, prog::ProgramGenerator, ActorId,
    CodeId, MessageId,
};

struct Contract {
    pub name: String,
    pub symbol: String,
    pub decimals: u64,
    pub balance_of: HashMap<ActorId, u128>,
    pub allowance: HashMap<(ActorId, ActorId), u128>,
    pub total_supply: u128,
}

static mut STATE: Option<Contract> = None;

impl Contract {
    async fn deposit(&mut self) -> Result<Event, Error> {
        let actor = msg::source();
        let amount = msg::value();
        let balance = self.balance_of.entry(actor).or_insert(0);
        *balance += amount;
        self.total_supply += amount;
        Ok(Event::Deposit {
            from: actor,
            amount,
        })
    }

    async fn withdraw(&mut self, to: ActorId, amount: u128) -> Result<Event, Error> {
        let actor = msg::source();
        let balance = self.balance_of.entry(actor).or_insert(0);
        if *balance < amount {
            return Err(Error::InsufficientBalance);
        }
        if self.total_supply < amount {
            return Err(Error::InsufficientTotalSupply);
        }
        self.total_supply -= amount;
        *balance -= amount;
        let send_res = msg::send_for_reply(to, {}, amount, 0);
        if send_res.is_err() {
            return Err(Error::SendFailed);
        }
        Ok(Event::Withdraw { to, amount })
    }

    async fn transfer_from(
        &mut self,
        src: ActorId,
        to: ActorId,
        amount: u128,
    ) -> Result<Event, Error> {
        let actor = msg::source();
        let balance = self.balance_of.entry(src).or_insert(0);
        if *balance < amount {
            return Err(Error::InsufficientBalance);
        }
        if src != actor {
            let allowance = self.allowance.entry((src, actor)).or_insert(0);
            if *allowance < amount {
                return Err(Error::InsufficientAllowance);
            }
            *allowance -= amount;
        }

        *balance -= amount;
        let to_balance = self.balance_of.entry(to).or_insert(0);
        *to_balance += amount;

        Ok(Event::Transfer {
            from: src,
            to,
            amount,
        })
    }

    async fn transfer(&mut self, to: ActorId, amount: u128) -> Result<Event, Error> {
        let actor = msg::source();
        self.transfer_from(actor, to, amount).await
    }

    async fn approve(&mut self, spender: ActorId, amount: u128) -> Result<Event, Error> {
        let actor = msg::source();
        self.allowance.insert((actor, spender), amount);
        Ok(Event::Approve {
            owner: actor,
            spender,
            amount,
        })
    }
}

#[no_mangle]
extern fn init() {
    let result = process_init();
    let is_err = result.is_err();

    reply(result).expect("failed to encode or reply from `init()`");

    if is_err {
        exec::exit(ActorId::zero());
    }
}

fn process_init() -> Result<(), Error> {
    unsafe {
        STATE = Some(Contract {
            name: "Wrapped Vara".to_string(),
            symbol: "WVARA".to_string(),
            decimals: 12,
            balance_of: HashMap::new(),
            allowance: HashMap::new(),
            total_supply: 0,
        });
    };
    Ok(())
}

#[gstd::async_main]
async fn main() {
    reply(process_handle().await).expect("failed to encode or reply `handle()`");
}

async fn process_handle() -> Result<Event, Error> {
    let action: Action = msg::load()?;
    let contract = state_mut();

    match action {
        Action::Deposit => contract.deposit().await,
        Action::Withdraw { to, amount } => contract.withdraw(to, amount).await,
        Action::Transfer { from ,to, amount } => contract.transfer(to, amount).await,
        Action::Approve { spender, amount } => contract.approve(spender, amount).await,
        Action::TransferFrom { from, to, amount } => contract.transfer_from(from, to, amount).await,
        Action::BalanceOf(actor) => Ok(Event::Balance(contract.balance_of.get(&actor).copied().unwrap_or_default())),
    }
}

fn state_mut() -> &'static mut Contract {
    unsafe { STATE.as_mut().expect("state isn't initialized") }
}

#[no_mangle]
extern fn state() {
    let Contract {
        name,
        symbol,
        decimals,
        balance_of,
        allowance,
        total_supply,
    } = state_mut();

    reply(WVaraState {
        name: name.clone(),
        symbol: symbol.clone(),
        decimals: *decimals,
        balance_of: balance_of
            .iter()
            .map(|(actor, balance)| (*actor, *balance))
            .collect(),
        allowance: allowance
            .iter()
            .map(|((owner, spender), amount)| ((*owner, *spender), *amount))
            .collect(),
        total_supply: *total_supply,
    })
    .expect("failed to encode or reply from `state()`");
}

fn reply(payload: impl Encode) -> Result<MessageId> {
    Ok(msg::reply(payload, 0)?)
}
