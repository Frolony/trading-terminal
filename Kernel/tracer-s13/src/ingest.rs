//! Event ingestion: the raw material of reconstruction.
//!
//! A [`TransferEvent`] is one observed value movement with its venue context
//! attached (program id + instruction family, as the indexer saw it). The
//! ingester sorts events into causal order — `(slot, index)` — and validates
//! the cheap invariants (non-negative amounts, non-empty endpoints) before
//! reconstruction ever sees them. Garbage in is rejected here, not
//! classified later.

/// One observed value movement.
///
/// Amounts are in the mint's base units (i128, signed only at the net-effect
/// layer — an event amount itself is never negative).
#[derive(Debug, Clone)]
pub struct TransferEvent {
    /// Transaction signature (the evidence; kept verbatim for the audit appendix).
    pub signature: String,
    /// Slot the transaction landed in.
    pub slot: u64,
    /// Index of the instruction within the transaction (causal order).
    pub index: u32,
    /// Milliseconds since epoch, as the indexer reported it.
    pub timestamp_ms: u64,
    pub from: String,
    pub to: String,
    /// Mint identifier (base58 address or a test label).
    pub mint: String,
    pub amount: i128,
    /// Program that executed the instruction.
    pub program_id: String,
    /// Instruction family name, as parsed by the indexer.
    pub instruction: String,
    /// Transaction fee in lamports, attributed to this event's transaction.
    pub fee_lamports: u64,
}

/// What the ingester rejects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestError {
    NegativeAmount { signature: String },
    EmptyEndpoint { signature: String },
}

impl TransferEvent {
    fn validate(&self) -> Result<(), IngestError> {
        if self.amount < 0 {
            return Err(IngestError::NegativeAmount { signature: self.signature.clone() });
        }
        if self.from.is_empty() || self.to.is_empty() {
            return Err(IngestError::EmptyEndpoint { signature: self.signature.clone() });
        }
        Ok(())
    }
}

/// Sort into causal order and validate. Returns the events in `(slot, index)`
/// order; the first invalid event aborts the batch with its signature.
pub fn ingest(mut events: Vec<TransferEvent>) -> Result<Vec<TransferEvent>, IngestError> {
    for e in &events {
        e.validate()?;
    }
    events.sort_by_key(|e| (e.slot, e.index));
    Ok(events)
}
