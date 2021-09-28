import {isTxError, LCDClient, MnemonicKey, MsgStoreCode} from '@terra-money/terra.js';
import * as fs from 'fs';

const mnemonic = '...';
const scCodePath = '../artifacts/fcqn.wasm'

// create a key out of a mnemonic
const mk = new MnemonicKey({
    mnemonic: mnemonic,
});

// connect to tequila testnet
const terra = new LCDClient({
    URL: 'https://tequila-lcd.terra.dev',
    chainID: 'tequila-0004'
});

const wallet = terra.wallet(mk);

async function main() {
    const storeCode = new MsgStoreCode(
        wallet.key.accAddress,
        fs.readFileSync(scCodePath).toString('base64')
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

main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error(error);
        process.exit(1);
    });