use alloy::eips::BlockId;
use alloy::network::Ethereum;
use alloy::providers::ProviderBuilder;
use alloy::providers::RootProvider;
use alloy::providers::WsConnect;
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::client::RpcClient;
use alloy::sol;
use alloy::sol_types::SolCall;
use alloy::transports::http::reqwest::Url;
use alloy::transports::http::Client;
use alloy::transports::http::Http;
use revm::db::AlloyDB;
use revm::db::CacheDB;
use revm::primitives::address;
use revm::primitives::TxEnv;
use revm::primitives::TxKind;
use revm::Evm;
use revm_contract::{calls, contract};
use std::sync::Arc;
use IERC20::balanceOfCall;
use IERC20::{allowanceCall, transferCall};

mod types;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    #[derive(Debug)]
    IERC20,
    "src/abi/IERC20.json"
);

type ExternalContexts = ();
type AlloyCacheDB = CacheDB<AlloyDB<Http<Client>, Ethereum, Arc<RootProvider<Http<Client>>>>>;

contract!(
    #[calls(allowanceCall, transferCall, balanceOfCall)]
    pub Erc20Contract<ExternalContexts, AlloyCacheDB>
);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let rpc_url: Url = "https://site1.moralis-nodes.com/ronin/d86f5e495de54c54a52d90699f413cc1"
        .parse()
        .unwrap();

    let provider = ProviderBuilder::new().on_http(rpc_url);

    let provider = Arc::new(provider);

    let db = AlloyDB::new(provider.clone(), BlockId::default()).unwrap();

    let cache_db: AlloyCacheDB = CacheDB::new(db);

    let mut evm: Evm<ExternalContexts, AlloyCacheDB> = Evm::builder().with_db(cache_db).build();

    let address = address!("0b7007c13325c48911f73a2dad5fa5dcbf808adc");

    let mut contract = Erc20Contract::new(address, &mut evm);

    let call = balanceOfCall {
        _owner: address!("c1eb47de5d549d45a871e32d9d082e7ac5d2e3ed"),
    };

    let tx_env = TxEnv::default();

    let v = contract.balance_of(call, tx_env);

    dbg!(v);

    Ok(())
}
