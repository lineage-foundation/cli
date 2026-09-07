//! Async data-fetch functions for the TUI. These are thin wrappers over
//! `lineage_sdk::Client` calls that shape the responses into the plain
//! data structs the `App` cache expects; kept free of any terminal/UI
//! concerns so they can be exercised directly with `wiremock`.

use lineage_sdk::models::{BalancesResponse, Supply};
use lineage_sdk::{Client, Result};

/// Aggregated data for the Dashboard tab.
pub struct DashboardData {
    pub head: u64,
    pub supply: Supply,
    pub wallet_total_raw: u64,
}

/// Fetches the latest block height, chain supply, and the summed token
/// balance across `addresses`.
pub async fn fetch_dashboard(client: &Client, addresses: &[String]) -> Result<DashboardData> {
    let latest = client.latest_block().await?;
    let head = latest
        .get("block")
        .and_then(|b| b.get("block"))
        .and_then(|b| b.get("header"))
        .and_then(|h| h.get("b_num"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);

    let supply = client.supply().await?;

    let addr_refs: Vec<&str> = addresses.iter().map(String::as_str).collect();
    let wallet_total_raw = client.balances(&addr_refs).await?.balance.total.tokens;

    Ok(DashboardData {
        head,
        supply,
        wallet_total_raw,
    })
}

/// Fetches the full balances response for the Wallet tab.
pub async fn fetch_wallet(client: &Client, addresses: &[String]) -> Result<BalancesResponse> {
    let addr_refs: Vec<&str> = addresses.iter().map(String::as_str).collect();
    client.balances(&addr_refs).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use lineage_sdk::Hosts;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn fetch_dashboard_returns_head_supply_and_wallet_total() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/blocks/latest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "block": {
                    "block": {
                        "header": {
                            "b_num": 7141u64
                        }
                    }
                }
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/v1/supply"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "total": 360_360_000_000_000_000u64,
                "issued": 720_720_000u64
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/v1/balances"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "balance": {
                    "address_list": {
                        "a1": [{"out_point": {"n": 0, "t_hash": "g59"}, "value": {"Token": 720720000}}]
                    },
                    "total": {"tokens": 720_720_000u64, "items": {}}
                }
            })))
            .mount(&server)
            .await;

        let hosts = Hosts {
            mempool: server.uri(),
            storage: server.uri(),
            miner: server.uri(),
        };
        let client = Client::new(hosts).unwrap();

        let data = fetch_dashboard(&client, &["a1".to_string()])
            .await
            .unwrap();

        assert_eq!(data.head, 7141);
        assert_eq!(data.supply.total, 360_360_000_000_000_000);
        assert_eq!(data.supply.issued, 720_720_000);
        assert_eq!(data.wallet_total_raw, 720_720_000);
    }
}
