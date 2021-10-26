import {isTxError, LCDClient, MnemonicKey, MsgStoreCode} from '@terra-money/terra.js';
import {readFileSync} from 'fs';
import {NetworkConfig} from "./config/network/config";
import {StoreCodeConfig} from "./config/storecode/config";

let networkConfig: NetworkConfig = require('./config/network/config.json');
let config: StoreCodeConfig = require('./config/storecode/config.json');

// create a key out of a mnemonic
const mk = new MnemonicKey({
  mnemonic: networkConfig.mnemonic,
});

// connect to tequila testnet
const terra = new LCDClient({
  URL: networkConfig.url,
  chainID: networkConfig.chainID,
});

const wallet = terra.wallet(mk);

async function StoreCode() {
  const storeCode = new MsgStoreCode(
    wallet.key.accAddress,
    readFileSync(config.codePath).toString('base64')
  );
  const storeCodeTx = await wallet.createAndSignTx({
    msgs: [storeCode],
  });
  const storeCodeTxResult = await terra.tx.broadcast(storeCodeTx);

  console.log(storeCodeTxResult);

  if (isTxError(storeCodeTxResult)) {
    throw new Error(
      `store code failed. code: ${storeCodeTxResult.code}, codespace: ${storeCodeTxResult.codespace}, raw_log: ${storeCodeTxResult.raw_log}`
    );
  }

  const {store_code: {code_id}} = storeCodeTxResult.logs[0].eventsByType;
  console.log(`code_id: ${code_id}`);
}

StoreCode()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
