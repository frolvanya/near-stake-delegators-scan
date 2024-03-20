use crate::methods;

use color_eyre::Result;
use near_jsonrpc_client::JsonRpcClient;
use std::collections::{BTreeMap, BTreeSet};

use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Default)]
pub struct ValidatorsWithTimestamp {
    pub timestamp: i64,
    pub validator_staking_pools: BTreeMap<String, BTreeSet<String>>,
}

impl From<&DelegatorsWithTimestamp> for ValidatorsWithTimestamp {
    fn from(delegators: &DelegatorsWithTimestamp) -> Self {
        let mut validators_map = BTreeMap::<String, BTreeSet<String>>::new();

        for (delegator, validators) in &delegators.delegator_staking_pools {
            for validator in validators {
                validators_map
                    .entry(validator.to_string())
                    .or_default()
                    .insert(delegator.clone());
            }
        }

        Self {
            timestamp: delegators.timestamp,
            validator_staking_pools: validators_map,
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Default, Clone)]
#[serde(crate = "rocket::serde")]
pub struct DelegatorsWithTimestamp {
    pub timestamp: i64,
    pub delegator_staking_pools: BTreeMap<String, BTreeSet<String>>,
}

impl From<&ValidatorsWithTimestamp> for DelegatorsWithTimestamp {
    fn from(validators: &ValidatorsWithTimestamp) -> Self {
        let mut delegators_map = BTreeMap::<String, BTreeSet<String>>::new();

        for (validator, delegators) in &validators.validator_staking_pools {
            for delegator in delegators {
                delegators_map
                    .entry(delegator.to_string())
                    .or_default()
                    .insert(validator.clone());
            }
        }

        Self {
            timestamp: validators.timestamp,
            delegator_staking_pools: delegators_map,
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Default, Clone)]
#[serde(crate = "rocket::serde")]
pub struct DelegatorWithTimestamp {
    pub timestamp: i64,
    pub delegator_staking_pools: BTreeSet<String>,
}

pub async fn update_delegators_by_validator_account_id(
    json_rpc_client: &JsonRpcClient,
    delegators_with_timestamp: &Arc<RwLock<DelegatorsWithTimestamp>>,
    validators_with_timestamp: &Arc<RwLock<ValidatorsWithTimestamp>>,
    validator_account_id: String,
    block_id: u64,
) -> Result<()> {
    info!(
        "Updating delegators for validator: {}",
        validator_account_id
    );

    let block_reference = near_primitives::types::BlockReference::BlockId(
        near_primitives::types::BlockId::Height(block_id),
    );

    for _ in 0..methods::ATTEMPTS {
        if let Ok(validator_delegators) = methods::get_delegators_by_validator_account_id(
            json_rpc_client,
            validator_account_id.clone(),
            block_reference.clone(),
        )
        .await
        {
            let timestamp = chrono::Utc::now().timestamp();
            let mut validators_with_timestamp = validators_with_timestamp.write().await;

            validators_with_timestamp.timestamp = timestamp;
            validators_with_timestamp
                .validator_staking_pools
                .insert(validator_account_id.clone(), validator_delegators);

            let updated_delegators_with_timestamp =
                DelegatorsWithTimestamp::from(&validators_with_timestamp.clone());
            drop(validators_with_timestamp);

            *delegators_with_timestamp.write().await = updated_delegators_with_timestamp.clone();

            info!("Updated delegators for validator: {}", validator_account_id);

            return Ok(());
        }

        warn!(
            "Failed to get delegators for validator_account_id: {}. Retrying...",
            validator_account_id
        );
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    color_eyre::eyre::bail!(
        "Failed to get delegators for validator_account_id: {}",
        validator_account_id
    )
}
