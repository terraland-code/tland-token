import {writeFileSync} from "fs";

interface AddressAmount {
  address: string,
  amount: string,
}

interface StakerSte {
  staker: string,
  ste: number,
}

// load stakers
let stakers: [StakerSte] = require('./files/stt_and_lp_stakers_for_TLAND.json')

console.log(stakers.length)

function CalculateAmounts() {
  let sum = 0;

  for (let i = 0; i < stakers.length; i++) {
    // minimum staked amount is 250
    if (stakers[i].ste > 250) {
      sum += Math.sqrt(stakers[i].ste)
    }
  }

  let r = 500000000000/sum

  let result: AddressAmount[] = []
  for (let i = 0; i < stakers.length; i++) {
    if (stakers[i].ste > 250) {
      let amount = Math.sqrt(stakers[i].ste) * r
      result.push({
        amount: Math.floor(amount).toString(),
        address: stakers[i].staker
      })
    }
  }

  let jsonData = JSON.stringify(result);
  writeFileSync("files/stt_and_lp_stakers.json", jsonData);
}

function showDuplicateAddresses(data: AddressAmount[]) {
  let myMap = new Map();
  for (let i = 0; i < data.length; i++) {
    if (myMap.has(data[i].address)) {
      console.log("repeated: ", data[i].address)
    }
    myMap.set(data[i].address, 0)
  }
}

function sumAmounts(data: AddressAmount[]) {
  let amount = 0
  for (let i = 0; i < data.length; i++) {
    amount = amount + parseInt(data[i].amount)
  }
  console.log(amount)
}

CalculateAmounts()

