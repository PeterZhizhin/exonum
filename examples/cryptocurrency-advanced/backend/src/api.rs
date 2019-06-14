// Copyright 2019 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Cryptocurrency API.

use exonum_merkledb::{ListProof, MapProof};

use exonum::{
    api::{self, ServiceApiBuilder, ServiceApiState},
    blockchain::{self, BlockProof, TransactionMessage},
    crypto::{Hash, PublicKey},
    explorer::BlockchainExplorer,
    helpers::Height,
    runtime::rust::{ServiceDescriptor, ServiceInstanceId},
};

use crate::{wallet::Wallet, Schema};

/// Describes the query parameters for the `get_wallet` endpoint.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct WalletQuery {
    /// Public key of the queried wallet.
    pub pub_key: PublicKey,
}

/// Proof of existence for specific wallet.
#[derive(Debug, Serialize, Deserialize)]
pub struct WalletProof {
    /// Proof of the whole database table.
    pub to_table: MapProof<Hash, Hash>,
    /// Proof of the specific wallet in this table.
    pub to_wallet: MapProof<PublicKey, Wallet>,
}

/// Wallet history.
#[derive(Debug, Serialize, Deserialize)]
pub struct WalletHistory {
    /// Proof of the list of transaction hashes.
    pub proof: ListProof<Hash>,
    /// List of above transactions.
    pub transactions: Vec<TransactionMessage>,
}

/// Wallet information.
#[derive(Debug, Serialize, Deserialize)]
pub struct WalletInfo {
    /// Proof of the last block.
    pub block_proof: BlockProof,
    /// Proof of the appropriate wallet.
    pub wallet_proof: WalletProof,
    /// History of the appropriate wallet.
    pub wallet_history: Option<WalletHistory>,
}

/// Public service API description.
#[derive(Debug, Clone)]
pub struct PublicApi {
    service_name: String,
    service_id: ServiceInstanceId,
}

impl PublicApi {
    /// Creates a new public API for the specified service instance.
    pub fn new(descriptor: ServiceDescriptor) -> Self {
        Self {
            service_name: descriptor.service_name().to_owned(),
            service_id: descriptor.service_id(),
        }
    }

    /// Endpoint for getting a single wallet.
    pub fn wallet_info(
        &self,
        state: &ServiceApiState,
        pub_key: PublicKey,
    ) -> api::Result<WalletInfo> {
        let snapshot = state.snapshot();
        let general_schema = blockchain::Schema::new(&snapshot);
        let currency_schema = Schema::new(&self.service_name, &snapshot);

        let max_height = general_schema.block_hashes_by_height().len() - 1;

        let block_proof = general_schema
            .block_and_precommits(Height(max_height))
            .unwrap();

        let to_table: MapProof<Hash, Hash> =
            general_schema.get_proof_to_service_table(self.service_id as u16, 0);

        let to_wallet: MapProof<PublicKey, Wallet> = currency_schema.wallets().get_proof(pub_key);

        let wallet_proof = WalletProof {
            to_table,
            to_wallet,
        };

        let wallet = currency_schema.wallet(&pub_key);

        let explorer = BlockchainExplorer::new(state.blockchain());

        let wallet_history = wallet.map(|_| {
            let history = currency_schema.wallet_history(&pub_key);
            let proof = history.get_range_proof(0..history.len());

            let transactions = history
                .iter()
                .map(|record| explorer.transaction_without_proof(&record).unwrap())
                .collect::<Vec<_>>();

            WalletHistory {
                proof,
                transactions,
            }
        });

        Ok(WalletInfo {
            block_proof,
            wallet_proof,
            wallet_history,
        })
    }

    /// Wires the above endpoint to public scope of the given `ServiceApiBuilder`.
    pub fn wire(self, builder: &mut ServiceApiBuilder) {
        builder.public_scope().endpoint(
            "v1/wallets/info",
            move |state: &ServiceApiState, query: WalletQuery| {
                self.wallet_info(state, query.pub_key)
            },
        );
    }
}
