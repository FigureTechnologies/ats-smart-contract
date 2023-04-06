# ATS profiler

This profiler allows you to quickly store and instantiate a smart contract,
as well as execute bid, ask, and match orders measuring the gas usage per
operation. 

## Prerequisites

1. Clone the [Provenance repository](https://github.com/provenance-io/provenance) and check out version [v1.14.1](https://github.com/provenance-io/provenance/releases/tag/v1.14.1) or higher:

  ```bash
  $ git clone git@github.com:provenance-io/provenance.git
  $ git checkout tags/v1.14.1
  ```

2. Clone the [ats-smart-contract repository](https://github.com/FigureTechnologies/ats-smart-contract):

  ```bash
  $ git@github.com:FigureTechnologies/ats-smart-contract.git
  ```

## Usage

0. Help is displayed by invoking the tool without arguments or by supplying the `--help` flag:

  ```bash
  $ ats-profiler

  >
  ATS Smart Contract profiler

  Usage: ats-profiler [OPTIONS] <COMMAND>

  Commands:
    store             Store a smart contract on chain
    instantiate       Instantiate a previously stored smart contract
    contract-version  View contract version
    contract-info     View contract information
    place-bid         Place a BID order
    place-ask         Place an ASK order
    execute-match     Execute an order match
    help              Print this message or the help of the given subcommand(s)

  Options:
    -v, --verbose <verbose>  Enable verbose output
    -h, --help               Print help
    -V, --version            Print version
  ```

1. Ensure that Provenance is built and localnet is running:

  ```bash
  $ cd provenance
  $ make clean
  $ make build
  $ make install
  $ make localnet-start
  ```

2. Once localnet is up and running, execute the setup script:

  ```bash
  $ ./setup.sh
  ```

3. Build the smart contract:

  ```
  $ cd ats-smart-contract
  $ cargo build
  ```

4. Build the profiler tool:

  ```bash
  $ cd $PROFILER_HOME
  $ cargo build
  $ cargo install --path .  # ensure $HOME/.cargo/bin/ exists and is part of $PATH
  ```

4. Store the smart contract WASM on chain using the profiler tool:

  ```bash
  $ ats-profiler store --wasm-dir ats-smart-contract/artifacts

  > {"code_id":1}
  ```

5. Instantiate the smart contract:

  ```bash
  $ ats-profiler instantiate --code-id 1

  > {"code_id": 1, "address": "tp14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9s96lrg8"}

  ```

6. Place a bid order:

  ```bash
  $ ats-profiler place-bid --code-id 1 --address tp14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9s96lrg8 | jq

  >
  Successfully submitted BID: ID = 0ec05211-bcfb-4d6c-8d7f-e97835d04006
  Profiler output:
  {
    "ats_smart_contract": {
      "Execute__bid { id = 0ec05211-bcfb-4d6c-8d7f-e97835d04006, base=gme.local, quote=usd.local, price=2, quote_size=1000, size=500 }": {
        "file_name": "src/main.rs",
        "gas_used": 171707,
        "gas_wanted": 235025,
        "line_number": 278
      }
    }
  }
  ```

7. Place an ask order:

  ```bash
  $ ats-profiler place-ask --code-id 1 --address tp14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9s96lrg8 | jq

  >
  Successfully submitted ASK: ID = a0674876-a8b3-446f-bfae-33053821a624
  Profiler output:
   {
    "ats_smart_contract": {
      "Execute__ask { id=a0674876-a8b3-446f-bfae-33053821a624, base=gme.local, quote=usd.local, price=2, size=500 }": {
        "file_name": "src/main.rs",
        "gas_used": 179764,
        "gas_wanted": 247106,
        "line_number": 315
      }
    }
  }
  ```

8. Execute a match using the previous bid and ask orders:

  ```bash
  $ ats-profiler --code-id 1 \
    --address tp14hj2tavq8fpesdwxxcu44rty3hh90vhujrvcmstl4zr3txmfvw9s96lrg8 \
    --bid-id=0ec05211-bcfb-4d6c-8d7f-e97835d04006 \
    --ask-id=a0674876-a8b3-446f-bfae-33053821a624 | jq

  >
  Profiler output:
  {
    "ats_smart_contract": {
      "Execute__match { bid_id=0ec05211-bcfb-4d6c-8d7f-e97835d04006, ask_id=a0674876-a8b3-446f-bfae-33053821a624, price=2, size=500 }": {
        "file_name": "src/main.rs",
        "gas_used": 185191,
        "gas_wanted": 254954,
        "line_number": 350
      }
    }
  }
  ```
