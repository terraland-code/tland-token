import {isTxError, MsgStoreCode, Wallet} from '@terra-money/terra.js';
import {readFileSync, writeFileSync} from 'fs';
import {terra, token_owner_wallet} from './keys';

async function StoreCode(wallet: Wallet, codePath: string) {
  const storeCode = new MsgStoreCode(
    wallet.key.accAddress,
    readFileSync(codePath).toString('base64')
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

  return code_id[0];
}

async function StoreCodes() {
  let token_code_id = await StoreCode(token_owner_wallet, "../../artifacts/tland_token.wasm")
  let staking_code_id = await StoreCode(token_owner_wallet,"../../artifacts/staking.wasm")
  let airdrop_code_id = await StoreCode(token_owner_wallet,"../../artifacts/airdrop.wasm")
  let vesting_code_id = await StoreCode(token_owner_wallet,"../../artifacts/vesting.wasm")

  let code_ids = {
    token_code_id: token_code_id,
    staking_code_id: staking_code_id,
    airdrop_code_id: airdrop_code_id,
    vesting_code_id: vesting_code_id,
  }

  let jsonData = JSON.stringify(code_ids);
  writeFileSync("files/code_ids.json", jsonData);
}

StoreCodes()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
