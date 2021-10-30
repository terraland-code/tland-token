import {LCDClient, MnemonicKey, isTxError, MsgExecuteContract} from '@terra-money/terra.js';
import {NetworkConfig} from "./config/network/config";

let networkConfig: NetworkConfig = require('./config/network/config.json');

const token_address = "terra18nuhtf4ajudu7hvslxf87cw7e4djdejwtqfe6u";
const terraswap_factory_address = "terra18qpjm4zkvqnpjpw0zn0tdr8gdzvt8au35v45xf";

// create a key out of a mnemonic
const mk = new MnemonicKey({
    mnemonic: networkConfig.mnemonic,
});

// connect to tequila testnet
const terra = new LCDClient({
    URL: networkConfig.url,
    chainID: networkConfig.chainID
});

const wallet = terra.wallet(mk);

async function main() {
    const execute = new MsgExecuteContract(
        wallet.key.accAddress, // sender
        terraswap_factory_address, // contract address
        {
            create_pair: {
                asset_infos: [
                    {
                        token: {
                            contract_addr: token_address
                        }
                    },
                    {
                        native_token: {
                            denom: "uusd"
                        }
                    }
                ]
            }
        }, // message
        undefined // coins
    );

    const executeTx = await wallet.createAndSignTx({
        msgs: [execute]
    });

    const executeTxResult = await terra.tx.broadcast(executeTx);

    console.log(executeTxResult);

    if (isTxError(executeTxResult)) {
        throw new Error(
            `execute failed. code: ${executeTxResult.code}, codespace: ${executeTxResult.codespace}, raw_log: ${executeTxResult.raw_log}`
        );
    }
}

main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error(error);
        process.exit(1);
    });
