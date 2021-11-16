import {isTxError, MsgInstantiateContract, Wallet} from '@terra-money/terra.js';
import {
  advisors_owner_wallet, airdrop_owner_wallet,
  delay,
  devfund_owner_wallet, lp_owner_wallet,
  privsale_owner_wallet, pubsale_owner_wallet, staking_owner_wallet, team_owner_wallet,
  terra,
  token_owner_wallet
} from './keys';
import {writeFileSync} from "fs";
import {Config} from "./config/config";

let cfg: Config = require('./config/config.json');
let TGE = cfg.tge

interface CodeIds {
  token_code_id: string,
  staking_code_id: string,
  airdrop_code_id: string,
  vesting_code_id: string,
}

let code_ids: CodeIds = require('./files/code_ids.json');
let MONTH = 30 * 24 * 3600
let WEEK = 7 * 24 * 3600

async function Instantiate(wallet: Wallet, code_id: number, memo: string, msg: object) {
  const instantiate = new MsgInstantiateContract(
    wallet.key.accAddress, // sender
    wallet.key.accAddress, // admin
    code_id,
    msg, // InitMsg
    undefined, // init coins
  );

  const instantiateTx = await wallet.createAndSignTx({
    msgs: [instantiate],
    memo: memo
  });
  const instantiateTxResult = await terra.tx.broadcast(instantiateTx);

  console.log(instantiateTxResult);

  if (isTxError(instantiateTxResult)) {
    throw new Error(
      `instantiate failed. code: ${instantiateTxResult.code}, codespace: ${instantiateTxResult.codespace}, raw_log: ${instantiateTxResult.raw_log}`
    );
  }

  const {instantiate_contract: {contract_address}} = instantiateTxResult.logs[0].eventsByType;
  console.log(`contract_address: ${contract_address}`)

  return contract_address
}

async function InstantiateToken() {
  let token_address = await Instantiate(
    token_owner_wallet,
    parseInt(code_ids.token_code_id),
    "INSTANTIATE TERRALAND TLAND TOKEN",
    {
      owner: token_owner_wallet.key.accAddress,
      decimals: 6,
      name: "TerraLand token",
      symbol: "TLAND",
      marketing: {
        marketing: token_owner_wallet.key.accAddress
      },
      initial_balances: [
        // Treasury 25 000 000 TLAND
        {
          address: "terra1ly5glvd0xv5x5s4vd5x6a8p8n4pcmwn839pcep",
          amount: "25000000000000"
        },
        // Team 17 000 000 TLAND
        {
          address: "terra1hek3fzkmke5pe6lvcv48frwvchc04fz6y22fyj",
          amount: "17000000000000"
        },
        // Community incentives, Staking 13 000 000 TLAND
        {
          address: "terra1yjwlg6dy3dkq3qlhv0feyqe7wrt3q0h0ghlwcy",
          amount: "13000000000000"
        },
        // Private sale & marketing partnerships 20 000 000 TLAND
        {
          address: "terra1ksawlatvhqmm3lg9uc7w6z20zvvuegmwxgjtpm",
          amount: "20000000000000"
        },
        // Public sale Subme 1 111 555,000000 TLAND
        {
          address: "terra1wcuvasqx8zf69e9jhnnxgk4em7dnemqappugwj",
          amount: "1111555000000"
        },
        // Public sale Starterra 8 888 445,000000 TLAND
        {
          address: "terra1ylng8sxnkghx4grp3auvln6g5ugql7kw29d4h2",
          amount: "8888445000000"
        },
        // Development fund 5 000 000 TLAND
        {
          address: "terra1z43ptner54dvkpz2cuyu67wjkzmqzs2kq8wsu5",
          amount: "5000000000000",
        },
        // Advisors fund 4 000 000 TLAND
        {
          address: "terra1u4cukjhadget74ugc4antvca0v0jzlxxmjp5t7",
          amount: "4000000000000",
        },
        // Liquidity for smart contract 1 750 000 TLAND
        {
          address: "terra1w6402sdfcu4smfhunqvwyv6kwq87f2kvnc4z0m",
          amount: "1750000000000",
        },
        // Liquidity rest 3 250 000 TLAND
        {
          address: "terra1xr50nhz5ecqswaehnxf7f7nvfeu0zh7424vzwe",
          amount: "3250000000000",
        },
        // Airdrop 1 000 000 TLAND
        {
          address: "terra1amskskeaput62xdahpp59yrw4fm7g7ndgsc20u",
          amount: "1000000000000",
        }
      ]
    }
  )

  return token_address[0]
}

async function InstantiateDevFund(terraland_token: string) {
  let vesting_address = await Instantiate(
    devfund_owner_wallet,
    parseInt(code_ids.vesting_code_id),
    "INSTANTIATE TERRALAND DEVELOPMENT FUND VESTING",
    {
      owner: devfund_owner_wallet.key.accAddress,
      terraland_token: terraland_token,
      name: "TERRALAND_DEVELOPMENT_FUND_VESTING",
      fee_config: [
        {
          fee: "1000000",
          operation: "claim",
          denom: "uusd"
        }
      ],
      vesting: {
        start_time: TGE,
        end_time: TGE + 39 * MONTH,
        initial_percentage: 0,
        cliff_end_time: TGE + 3 * MONTH,
      }
    })

  return vesting_address[0]
}

async function InstantiateAdvisors(terraland_token: string) {
  let vesting_address = await Instantiate(
    advisors_owner_wallet,
    parseInt(code_ids.vesting_code_id),
    "INSTANTIATE TERRALAND ADVISORS VESTING",
    {
      owner: advisors_owner_wallet.key.accAddress,
      terraland_token: terraland_token,
      name: "TERRALAND_ADVISORS_VESTING",
      fee_config: [
        {
          fee: "1000000",
          operation: "claim",
          denom: "uusd"
        }
      ],
      vesting: {
        start_time: TGE,
        end_time: TGE + 13 * MONTH,
        initial_percentage: 0,
        cliff_end_time: TGE + MONTH,
      }
    })

  return vesting_address[0]
}

async function InstantiatePrivSale(terraland_token: string) {
  let vesting_address = await Instantiate(
    privsale_owner_wallet,
    parseInt(code_ids.vesting_code_id),
    "INSTANTIATE TERRALAND PRIVATE SALE VESTING",
    {
      owner: privsale_owner_wallet.key.accAddress,
      terraland_token: terraland_token,
      name: "TERRALAND_PRIVATE_SALE_VESTING",
      fee_config: [
        {
          fee: "1000000",
          operation: "claim",
          denom: "uusd"
        }
      ],
      vesting: {
        start_time: TGE,
        end_time: TGE + 10 * MONTH,
        initial_percentage: 10,
        cliff_end_time: TGE + MONTH,
      }
    })

  return vesting_address[0]
}

async function InstantiatePubSale(terraland_token: string) {
  let vesting_address = await Instantiate(
    pubsale_owner_wallet,
    parseInt(code_ids.vesting_code_id),
    "INSTANTIATE TERRALAND PUBLIC SALE VESTING",
    {
      owner: pubsale_owner_wallet.key.accAddress,
      terraland_token: terraland_token,
      name: "TERRALAND_PUBLIC_SALE_VESTING",
      fee_config: [
        {
          fee: "1000000",
          operation: "claim",
          denom: "uusd"
        }
      ],
      vesting: {
        start_time: TGE,
        end_time: TGE + 6 * MONTH,
        initial_percentage: 20,
        cliff_end_time: TGE,
      }
    })

  return vesting_address[0]
}

async function InstantiateTeam(terraland_token: string) {
  let vesting_address = await Instantiate(
    team_owner_wallet,
    parseInt(code_ids.vesting_code_id),
    "INSTANTIATE TERRALAND TEAM VESTING",
    {
      owner: team_owner_wallet.key.accAddress,
      terraland_token: terraland_token,
      name: "TERRALAND_TEAM_VESTING",
      fee_config: [
        {
          fee: "1000000",
          operation: "claim",
          denom: "uusd"
        }
      ],
      vesting: {
        start_time: TGE,
        end_time: TGE + 24 * MONTH,
        initial_percentage: 0,
        cliff_end_time: TGE + 6 * MONTH,
      }
    })

  return vesting_address[0]
}

async function InstantiateLpStaking(terraland_token: string) {
  let lp_staking_address = await Instantiate(
    staking_owner_wallet,
    parseInt(code_ids.staking_code_id),
    "INSTANTIATE TERRALAND LP STAKING",
    {
      owner: staking_owner_wallet.key.accAddress,
      staking_token: staking_owner_wallet.key.accAddress,
      terraland_token: terraland_token,
      unbonding_period: 432000, // 5 days
      burn_address: "terra17hk7d34mg77w6ujcr6n58p8hjl9ez8w9gj6auk",
      instant_claim_percentage_loss: 5, // 5%
      fee_config: [
        {
          fee: "1000000",
          operation: "claim",
          denom: "uusd"
        },
        {
          fee: "1000000",
          operation: "unbond",
          denom: "uusd"
        },
        {
          fee: "1000000",
          operation: "instant_claim",
          denom: "uusd"
        },
        {
          fee: "1000000",
          operation: "withdraw",
          denom: "uusd"
        }
      ],
      distribution_schedule: [
        {
          amount: "600000000000",
          start_time: TGE,
          end_time: TGE+4*WEEK
        },
        {
          amount: "202000000000",
          start_time: TGE+4*WEEK,
          end_time: TGE+6*WEEK
        },
        {
          amount: "205500000000",
          start_time: TGE+6*WEEK,
          end_time: TGE+8*WEEK
        },
        {
          amount: "210000000000",
          start_time: TGE+8*WEEK,
          end_time: TGE+10*WEEK
        },
        {
          amount: "430000000000",
          start_time: TGE+10*WEEK,
          end_time: TGE+14*WEEK
        },
        {
          amount: "440000000000",
          start_time: TGE+14*WEEK,
          end_time: TGE+18*WEEK
        },
        {
          amount: "900000000000",
          start_time: TGE+18*WEEK,
          end_time: TGE+26*WEEK
        },
        {
          amount: "920000000000",
          start_time: TGE+26*WEEK,
          end_time: TGE+34*WEEK
        },
        {
          amount: "1527500000000",
          start_time: TGE+34*WEEK,
          end_time: TGE+47*WEEK
        },
        {
          amount: "980000000000",
          start_time: TGE+47*WEEK,
          end_time: TGE+55*WEEK
        },
        {
          amount: "1560000000000",
          start_time: TGE+55*WEEK,
          end_time: TGE+68*WEEK
        },
        {
          amount: "1495000000000",
          start_time: TGE+68*WEEK,
          end_time: TGE+81*WEEK
        },
        {
          amount: "2530000000000",
          start_time: TGE+81*WEEK,
          end_time: TGE+104*WEEK
        },
      ]
    })

  return lp_staking_address[0]
}


async function InstantiateAirdrop(terraland_token: string, lp_staking_address: string) {
  let airdrop_address = await Instantiate(
    airdrop_owner_wallet,
    parseInt(code_ids.airdrop_code_id),
    "INSTANTIATE TERRALAND AIRDROP",
    {
      owner: airdrop_owner_wallet.key.accAddress,
      terraland_token: terraland_token,
      fee_config: [
        {
          fee: "1000000",
          operation: "claim",
          denom: "uusd"
        }
      ],
      mission_smart_contracts: {
        lp_staking: lp_staking_address
      }
    })

  return airdrop_address[0]
}

async function DeployContracts() {
  let token_address = await InstantiateToken()
  delay(10000)
  let devfund_address = await InstantiateDevFund(token_address)
  delay(10000)
  let advisors_address = await InstantiateAdvisors(token_address)
  delay(10000)
  let privsale_address = await InstantiatePrivSale(token_address)
  delay(10000)
  let pubsale_address = await InstantiatePubSale(token_address)
  delay(10000)
  let team_address = await InstantiateTeam(token_address)
  delay(10000)
  let lp_staking_address = await InstantiateLpStaking(token_address)
  delay(10000)
  let airdrop_address = await InstantiateAirdrop(token_address, lp_staking_address)

  let contract_addresses = {
    token_address: token_address,
    devfund_address: devfund_address,
    advisors_address: advisors_address,
    privsale_address: privsale_address,
    pubsale_address: pubsale_address,
    team_address: team_address,
    lp_staking_address: lp_staking_address,
    airdrop_address: airdrop_address,
  }

  let jsonData = JSON.stringify(contract_addresses);
  writeFileSync("files/contract_addresses.json", jsonData);
}

DeployContracts()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
