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

export const token_owner_wallet = terra.wallet(token_owner_key);

