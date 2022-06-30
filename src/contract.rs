use std::str::FromStr;

use bech32::{ToBase32, FromBase32};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{
    Bech32AddrResponse, BytesAddrResponse, CountResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
};
use crate::state::{State, STATE};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:counter-1-0";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        count: msg.count,
        owner: info.sender.clone(),
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender)
        .add_attribute("count", msg.count.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Increment {} => try_increment(deps),
        ExecuteMsg::Reset { count } => try_reset(deps, info, count),
    }
}

pub fn try_increment(deps: DepsMut) -> Result<Response, ContractError> {
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.count += 1;
        Ok(state)
    })?;

    Ok(Response::new().add_attribute("method", "try_increment"))
}

pub fn try_reset(deps: DepsMut, info: MessageInfo, count: i32) -> Result<Response, ContractError> {
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        if info.sender != state.owner {
            return Err(ContractError::Unauthorized {});
        }
        state.count = count;
        Ok(state)
    })?;
    Ok(Response::new().add_attribute("method", "reset"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetCount {} => to_binary(&query_count(deps)?),
        QueryMsg::ToBech32 { prefix, bytes } => to_binary(&to_bech32_addr(deps, prefix, bytes)?),
        QueryMsg::FromBech32 { bech32 } => to_binary(&from_bech32_addr(deps, bech32)?),
    }
}

fn query_count(deps: Deps) -> StdResult<CountResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(CountResponse { count: state.count })
}

fn to_bech32_addr(_deps: Deps, prefix: String, bytes: [u8; 32]) -> StdResult<Bech32AddrResponse> {
    let bech32_addr = bech32::encode(&prefix, bytes.to_vec().to_base32(), bech32::Variant::Bech32).unwrap();
    Ok(Bech32AddrResponse{
        bech32_addr,
    })
}

fn from_bech32_addr(_deps: Deps, bech32_addr: String) -> StdResult<BytesAddrResponse> {
    let (prefix, data, _) = bech32::decode(&bech32_addr).unwrap();
    let data = Vec::<u8>::from_base32(&data).unwrap();

    let mut bytes = [0u8; 32];
    bytes
        .iter_mut()
        .zip(&data)
        .for_each(|(b1, b2)| *b1 = *b2);

    Ok(BytesAddrResponse { prefix, bytes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(17, value.count);
    }

    #[test]
    fn increment() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Increment {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // should increase counter by 1
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(18, value.count);
    }

    #[test]
    fn reset() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let unauth_info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let res = execute(deps.as_mut(), mock_env(), unauth_info, msg);
        match res {
            Err(ContractError::Unauthorized {}) => {}
            _ => panic!("Must return unauthorized error"),
        }

        // only the original creator can reset the counter
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // should now be 5
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(5, value.count);
    }

    #[test]
    fn test_addr_conversions() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Mock data(obtained from "fromBech32" & "toBech32" in `cosmjs/encoding` npm pkg)
        let mock_beck32_addr = "juno1lqgdq9u8zhcvwwwz3xjswactrtq6qzptmlzlh6xspl34dxq32uhqhlphat";
        let mock_prefix = "juno".to_string();
        let mock_bytes: [u8; 32] = [
            248, 16, 208, 23, 135, 21, 240, 199, 57, 194, 137, 165, 7, 119, 11, 26, 193, 160, 8,
            43, 223, 197, 251, 232, 208, 15, 227, 86, 152, 17, 87, 46,
        ];

        // Check "FromBech32"
        let res = query(deps.as_ref(), mock_env(), QueryMsg::FromBech32 { bech32: mock_beck32_addr.to_string() }).unwrap();
        let bytes_addr_resp: BytesAddrResponse = from_binary(&res).unwrap();
        assert_eq!(bytes_addr_resp.prefix, mock_prefix.clone());
        assert_eq!(bytes_addr_resp.bytes, mock_bytes);

        // Check "ToBech32"
        let res = query(deps.as_ref(), mock_env(), QueryMsg::ToBech32 { prefix: mock_prefix, bytes: mock_bytes }).unwrap();
        let bech32_addr_resp: Bech32AddrResponse = from_binary(&res).unwrap();
        assert_eq!(bech32_addr_resp.bech32_addr, mock_beck32_addr.to_string());
    }
}
