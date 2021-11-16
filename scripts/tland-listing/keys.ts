import {LCDClient, MnemonicKey} from '@terra-money/terra.js';
import {Config} from "./config/config";
import * as dotenv from "dotenv";

let cfg: Config = require('./config/config.json');
dotenv.config();

// connect to tequila testnet
export const terra = new LCDClient({
  URL: cfg.url,
  chainID: cfg.chainID,
});

// create a key out of a mnemonic
const token_owner_key = new MnemonicKey({mnemonic: process.env.TERRALAND_TOKEN});
const pubsale_owner_key = new MnemonicKey({mnemonic:process.env.TERRALAND_PUBSALE});
const staking_owner_key = new MnemonicKey({mnemonic:process.env.TERRALAND_STAKING});
const privsale_owner_key = new MnemonicKey({mnemonic:process.env.TERRALAND_PRIVSALE});
const airdrop_owner_key = new MnemonicKey({mnemonic:process.env.TERRALAND_AIRDROP});
const lp_owner_key = new MnemonicKey({mnemonic:process.env.TERRALAND_LP});
const team_owner_key = new MnemonicKey({mnemonic:process.env.TERRALAND_TEAM});
const devfund_owner_key = new MnemonicKey({mnemonic:process.env.TERRALAND_DEVFUND});
const advisors_owner_key = new MnemonicKey({mnemonic:process.env.TERRALAND_ADVISORS});

export const token_owner_wallet = terra.wallet(token_owner_key);
export const pubsale_owner_wallet = terra.wallet(pubsale_owner_key);
export const staking_owner_wallet = terra.wallet(staking_owner_key);
export const privsale_owner_wallet = terra.wallet(privsale_owner_key);
export const airdrop_owner_wallet = terra.wallet(airdrop_owner_key);
export const lp_owner_wallet = terra.wallet(lp_owner_key);
export const team_owner_wallet = terra.wallet(team_owner_key);
export const devfund_owner_wallet = terra.wallet(devfund_owner_key);
export const advisors_owner_wallet = terra.wallet(advisors_owner_key);

export function delay(ms: number) {
  return new Promise( resolve => setTimeout(resolve, ms) );
}

