use async_trait::async_trait;

#[async_trait]
pub trait TransactionTrait: Send + Sync {
    type Error: Send + Sync;
    async fn commit(&mut self) -> Result<(), Self::Error>;
    async fn rollback(&mut self) -> Result<(), Self::Error>;
}

#[async_trait]
pub trait TransactionManager: Send + Sync {
    type Error: Send + Sync;
    type Tx: TransactionTrait<Error = Self::Error> + Send + Sync;
    type Connection: Send + Sync;

    async fn begin_transaction(&mut self) -> Result<Self::Tx, Self::Error>;
    async fn commit_transaction(tx: &mut Self::Tx) -> Result<(), Self::Error>;
    async fn rollback_transaction(tx: &mut Self::Tx) -> Result<(), Self::Error>;
}
