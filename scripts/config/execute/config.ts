import { Coins } from '@terra-money/terra.js';

export interface ExecuteConfig
{
  contractAddress: string;
  msg: object;
  coins: Coins;
}
