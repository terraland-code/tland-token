import {isTxError, MsgExecuteContract, Msg, Wallet} from '@terra-money/terra.js';
import {
  advisors_owner_wallet, airdrop_owner_wallet,
  delay,
  devfund_owner_wallet,
  privsale_owner_wallet, pubsale_owner_wallet,
  team_owner_wallet,
  terra
} from "./keys";

interface AddressAmount {
  address: string,
  amount: string,
}

interface ContractAddresses {
  token_address: string,
  devfund_address: string,
  advisors_address: string,
  privsale_address: string,
  pubsale_address: string,
  team_address: string,
  lp_staking_address: string,
  airdrop_address: string,
}

let devfund_members: [AddressAmount] = require('./files/devfund_members.json')
let team_members: [AddressAmount] = require('./files/team_members.json')
let advisors_members: [AddressAmount] = require('./files/advisors_members.json')
let privsale_members: [AddressAmount] = require('./files/privsale_members.json')
let pubsale_members: [AddressAmount] = require('./files/pubsale_members.json')
let airdrop_members_stt: [AddressAmount] = require('./files/stt_and_lp_stakers.json')

let contract_addresses: ContractAddresses = require('./files/contract_addresses.json')

function SplitToMessages(data: [AddressAmount], wallet: Wallet, constract_address: string) {
  let msgs: object[][] = []

  for (let i = 0; i < data.length; i++) {
    if (i % 40 == 0) {
      msgs.push([
        {
          address: data[i].address,
          amount: data[i].amount,
          claimed: "0"
        }
      ])
    } else {
      let index = Math.floor(i/40)
      msgs[index].push(
        {
          address: data[i].address,
          amount: data[i].amount,
          claimed: "0"
        }
      )
    }
  }

  let execute_msgs: Msg[] = []
  for (let i = 0; i < msgs.length; i++) {
    let execute = new MsgExecuteContract(
      wallet.key.accAddress, // sender
      constract_address, // contract address
      {
        "register_members": msgs[i]
      }, // message
      undefined, // coins
    )
    execute_msgs.push(execute)
  }

  return execute_msgs
}

async function SendBatchMessages(msgs: Msg[], wallet: Wallet, memo: string) {
  let execute_msgs: Msg[] = []

  for (let i = 0; i < msgs.length; i++) {
    execute_msgs.push(msgs[i])

    if (i%25 == 0 && i>0) {
      const executeTx = await wallet.createAndSignTx({
        msgs: execute_msgs,
        memo: memo + " #" + Math.floor(i/25)
      });

      const executeTxResult = await terra.tx.broadcast(executeTx);
      if (isTxError(executeTxResult)) {
        throw new Error(
          `execute failed. code: ${executeTxResult.code}, codespace: ${executeTxResult.codespace}, raw_log: ${executeTxResult.raw_log}`
        );
      }
      console.log("memo:", executeTx.memo, "tx_hash: ", executeTxResult.txhash)

      // reset execute_msgs
      execute_msgs = []

      delay(10000)
    }
  }

  if (execute_msgs.length > 0) {
    const executeTx = await wallet.createAndSignTx({
      msgs: execute_msgs,
      memo: memo
    });

    const executeTxResult = await terra.tx.broadcast(executeTx);
    if (isTxError(executeTxResult)) {
      throw new Error(
        `execute failed. code: ${executeTxResult.code}, codespace: ${executeTxResult.codespace}, raw_log: ${executeTxResult.raw_log}`
      );
    }
    console.log("memo:", executeTx.memo, "tx_hash: ", executeTxResult.txhash)
  }
}

async function RegisterMembers(data: [AddressAmount], wallet: Wallet, constract_address: string, memo:string) {
  let msgs = SplitToMessages(data, wallet, constract_address)
  await SendBatchMessages(msgs, wallet, memo)
}

async function RegisterMembersForAllContracts() {
  await RegisterMembers(devfund_members, devfund_owner_wallet, contract_addresses.devfund_address,
    "REGISTER DEVELOPMENT FUND ADDRESSES")
  delay(10000)
  await RegisterMembers(team_members, team_owner_wallet, contract_addresses.team_address,
    "REGISTER TEAM ADDRESSES")
  delay(10000)
  await RegisterMembers(advisors_members, advisors_owner_wallet, contract_addresses.advisors_address,
    "REGISTER ADVISORS ADDRESSES")
  delay(10000)
  await RegisterMembers(privsale_members, privsale_owner_wallet, contract_addresses.privsale_address,
    "REGISTER PRIVATE SALE ADDRESSES")
  delay(10000)
  await RegisterMembers(pubsale_members, pubsale_owner_wallet, contract_addresses.pubsale_address,
    "REGISTER PUBLIC SALE ADDRESSES")
  delay(10000)
  await RegisterMembers(airdrop_members_stt, airdrop_owner_wallet, contract_addresses.airdrop_address,
    "REGISTER AIRDROP ADDRESSES")
}

RegisterMembersForAllContracts()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
