use std::{ops::Deref, sync::Arc, time::Duration};

use bytes::Bytes;
use p2p::PeerIdWithMultiaddr;

use crate::{db::{Db, DbDropGuard}, P2pClient};

#[derive(Debug, Clone)]
pub struct Node {
    inner: Arc<NodeInner>,
}

impl Node {
    pub fn new(
        db_holder: DbDropGuard,
        p2p: P2pClient,
    ) -> Self {
        Self {
            inner: Arc::new(NodeInner {
                db_holder,
                p2p,
            }),
        }
    }
}

impl Deref for Node {
    type Target = NodeInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Debug, Clone)]
pub struct NodeInner {
    // A state machine that holds the state of the blockchain.
    db_holder: DbDropGuard,

    p2p: P2pClient,
}

impl Node {
    pub fn db(&self) -> Db {
        self.db_holder.db()
    }

    pub(crate) fn get(&self, key: &str) -> Option<Bytes> {
        self.db().get(key)
    }

    // pub(crate) fn get_node_status(&self, key: &str) -> Option<Bytes> {
    //     self.p2p.get_node_status()
    // }

    pub(crate) fn set(&self, key: String, value: Bytes, expire: Option<Duration>) {
        self.db().set(key, value, expire)
    }

    // pub fn next_account_nonce(&self, account: &str) -> u64 {
    //     self.state.next_account_nonce(account)
    // }

    pub async fn peer_basic(&self) -> Option<Vec<String>> {
        let known_peers = self.p2p.get_known_peers().await;
        if known_peers.len() > 0 {
            Some(known_peers)
        } else {
            None
        }
    }

    pub async fn get_closet_peer(&self, key: String) -> Option<PeerIdWithMultiaddr> {
        let addr: PeerIdWithMultiaddr = "/ip".parse().unwrap();
    }

    // pub fn transfer(&self, from: &str, to: &str, value: u64, nonce: u64) -> Result<(), Error> {
    //     let tx = Tx::new(from, to, value, nonce);
    //     let signed_tx = self.sign_tx(tx)?;
    //     let _ = self.tx_sender.send(TxMsg {
    //         tx: signed_tx,
    //         need_broadcast: true,
    //     });

    //     Ok(())
    // }

    // pub fn get_blocks(&self, from_number: u64) -> Vec<Block> {
    //     self.state.get_blocks(from_number)
    // }

    // pub fn get_block(&self, number: u64) -> Option<Block> {
    //     self.state.get_block(number)
    // }

    // pub fn get_balances(&self) -> HashMap<String, u64> {
    //     self.state.get_balances()
    // }

    // pub fn block_height(&self) -> u64 {
    //     self.state.block_height()
    // }

    // pub fn last_block_hash(&self) -> Option<Hash> {
    //     self.state.last_block_hash()
    // }

    // pub fn handle_broadcast_block(&self, block: Block) {
    //     let _ = self.block_sender.send(block);
    // }

    // pub fn handle_broadcast_tx(&self, tx: SignedTx) {
    //     let _ = self.tx_sender.send(TxMsg {
    //         tx,
    //         need_broadcast: false,
    //     });
    // }

    // // Sign a transaction on behalf of users.
    // fn sign_tx(&self, tx: Tx) -> Result<SignedTx, Error> {
    //     let sig = self.wallet.sign(&tx.as_bytes(), &tx.from)?;

    //     Ok(SignedTx {
    //         tx: Some(tx),
    //         sig: sig.to_vec(),
    //     })
    // }
}
