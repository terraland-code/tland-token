import {LCDClient, MnemonicKey, MsgExecuteContract, isTxError} from '@terra-money/terra.js';
import { config } from './config/network/config.json';

const pool_address = "terra1jh0tjgmqwrml0te43j3s8zxwnr33tj95u44xja";

// create a key out of a mnemonic
const mk = new MnemonicKey({
    mnemonic: config.mnemonic,
});

// connect to tequila testnet
const terra = new LCDClient({
    URL: 'https://bombay-lcd.terra.dev',
    chainID: 'bombay-12'
});

const wallet = terra.wallet(mk);

async function main() {
    const execute = new MsgExecuteContract(
        wallet.key.accAddress, // sender
        pool_address, // contract address
        {
            swap: {
                offer_asset: {
                    info: {
                        native_token: {
                            denom: "uusd"
                        }
                    },
                    amount: "100000000"
                }
            }
        }, // message
        { uusd: "100000000" } // coins
    );

    const executeTx = await wallet.createAndSignTx({
        msgs: [execute]
    });

    const executeTxResult = await terra.tx.broadcast(executeTx);

    console.log(executeTxResult);

    // console.log(await terra.wasm.contractQuery(pool_address, {pair:{}}))
    // console.log(await terra.wasm.contractQuery(pool_address, {pool:{}}))
    // console.log(await terra.wasm.contractQuery(pool_address, {
    //     simulation: {
    //         offer_asset: {
    //             info: {
    //                 token: {
    //                     contract_addr: token_address
    //                 }
    //             },
    //             amount: "100000000"
    //         }
    //     }
    // }))
    // console.log(await terra.wasm.contractQuery(pool_address, {
    //     simulation: {
    //         offer_asset: {
    //             info: {
    //                 native_token: {
    //                     denom: "uusd"
    //                 }
    //             },
    //             amount: "100000000"
    //         }
    //     }
    // }))
}

main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error(error);
        process.exit(1);
    });
