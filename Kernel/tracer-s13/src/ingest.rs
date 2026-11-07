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

// ---------------------------------------------------------------------------
// Own-program event adapter (W3.3 Phase 5 S13 dogfood)
// ---------------------------------------------------------------------------

/// Label for the program-owned SOL vault as a transfer endpoint.
///
/// The value-moving events that pay OUT of the treasury (`TranchePaid`,
/// `Exited`, `LoanIssued`, `WhaleTrancheExecuted`) do not carry the vault
/// pubkey — the vault is an account of the instruction, not a field of the
/// event — so the adapter labels the source rather than inventing an
/// address. The exact vault is recoverable from the event's signature
/// (the audit appendix keeps it verbatim). The deposit events DO carry the
/// vault, so they use the real address.
pub const TREASURY_VAULT_LABEL: &str = "treasury-vault";

/// Label for the program-owned fee vault as a transfer endpoint.
/// Same convention as [`TREASURY_VAULT_LABEL`]: `FeesCollected` carries the
/// operator but not the vault.
pub const FEE_VAULT_LABEL: &str = "fee-vault";

/// One decoded field of a treasury-program event, as produced by the
/// extractor (`hidden_files/treasury-test-2026-11-06/extract-events.py`,
/// which decodes the deployed IDL's 37 events from devnet transaction
/// logs). Values are already decoded from borsh; pubkeys are base58,
/// 32-byte hashes are hex.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldValue<'a> {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    I64(i64),
    Bool(bool),
    Pubkey(&'a str),
    Hash(&'a str),
}

/// One decoded treasury-program event: the extractor's output row, in
/// the shape the adapter consumes. `index` is the event's order among the
/// transaction's decoded events (causal order within the transaction).
#[derive(Debug, Clone)]
pub struct ProgramEvent<'a> {
    pub signature: &'a str,
    pub slot: u64,
    pub index: u32,
    pub timestamp_ms: u64,
    pub fee_lamports: u64,
    pub name: &'a str,
    pub fields: &'a [(&'a str, FieldValue<'a>)],
}

fn u64_field<'a>(fields: &'a [(&'a str, FieldValue<'a>)], name: &str) -> Option<u64> {
    fields.iter().find(|(n, _)| *n == name).and_then(|(_, v)| match v {
        FieldValue::U64(x) => Some(*x),
        _ => None,
    })
}

fn pubkey_field<'a>(fields: &'a [(&'a str, FieldValue<'a>)], name: &str) -> Option<&'a str> {
    fields.iter().find(|(n, _)| *n == name).and_then(|(_, v)| match v {
        FieldValue::Pubkey(s) => Some(*s),
        _ => None,
    })
}

fn transfer(
    program_id: &str,
    event: &ProgramEvent<'_>,
    from: String,
    to: String,
    mint: String,
    amount: u64,
    instruction: &str,
) -> TransferEvent {
    TransferEvent {
        signature: event.signature.to_string(),
        slot: event.slot,
        index: event.index,
        timestamp_ms: event.timestamp_ms,
        from,
        to,
        mint,
        amount: amount as i128,
        program_id: program_id.to_string(),
        instruction: instruction.to_string(),
        fee_lamports: event.fee_lamports,
    }
}

/// Map one decoded treasury-program event to the value movement it
/// records, or `None` when the event moves no value.
///
/// The seven value-moving events (field names are the deployed IDL's):
///
/// * `TreasurySolDeposit` — depositor -> vault, SOL, `amount_raw`.
/// * `TreasuryDeposit` — depositor -> vault, the deposited `mint`,
///   `amount_raw`.
/// * `TranchePaid` — vault -> holder, SOL, `amount_raw` (the dividend).
/// * `Exited` — vault -> holder, SOL, `paid_raw` — the penalty
///   (`penalty_raw`) stays in the vault, so it is NOT part of the movement.
/// * `LoanIssued` — vault -> holder, SOL, `amount_raw`.
/// * `FeesCollected` — fee vault -> operator, SOL, `amount_raw`.
/// * `WhaleTrancheExecuted` — vault -> holder, SOL, `amount_raw`.
///
/// Everything else returns `None`, and the reason is structural, not a
/// guess: `AllocationMade`, `BorrowReinvested`, `Reinvested`, and
/// `DefaultSettled` are bookkeeping-only (no tokens move — `AllocationMade`
/// is asserted bookkeeping-only by the W3.3 Phase 2 run; the other three
/// move position/accrual state, never lamports); the remaining events are
/// governance/lifecycle (`Committed`, `Settled`, `SignalSubmitted`, ...).
/// A value-moving event missing a required field also returns `None` —
/// fail-closed, never a guessed transfer.
pub fn program_event_to_transfer(
    program_id: &str,
    event: &ProgramEvent<'_>,
) -> Option<TransferEvent> {
    let f = event.fields;
    match event.name {
        "TreasurySolDeposit" => {
            let depositor = pubkey_field(f, "depositor")?;
            let vault = pubkey_field(f, "vault")?;
            let amount = u64_field(f, "amount_raw")?;
            Some(transfer(
                program_id,
                event,
                depositor.to_string(),
                vault.to_string(),
                "SOL".to_string(),
                amount,
                "deposit_sol",
            ))
        }
        "TreasuryDeposit" => {
            let depositor = pubkey_field(f, "depositor")?;
            let vault = pubkey_field(f, "vault")?;
            let mint = pubkey_field(f, "mint")?;
            let amount = u64_field(f, "amount_raw")?;
            Some(transfer(
                program_id,
                event,
                depositor.to_string(),
                vault.to_string(),
                mint.to_string(),
                amount,
                "deposit",
            ))
        }
        "TranchePaid" => {
            let holder = pubkey_field(f, "holder")?;
            let amount = u64_field(f, "amount_raw")?;
            Some(transfer(
                program_id,
                event,
                TREASURY_VAULT_LABEL.to_string(),
                holder.to_string(),
                "SOL".to_string(),
                amount,
                "pay_tranche",
            ))
        }
        "Exited" => {
            let holder = pubkey_field(f, "holder")?;
            // paid_raw, not amount_raw: the 13% penalty stays in the vault.
            let paid = u64_field(f, "paid_raw")?;
            Some(transfer(
                program_id,
                event,
                TREASURY_VAULT_LABEL.to_string(),
                holder.to_string(),
                "SOL".to_string(),
                paid,
                "exit",
            ))
        }
        "LoanIssued" => {
            let holder = pubkey_field(f, "holder")?;
            let amount = u64_field(f, "amount_raw")?;
            Some(transfer(
                program_id,
                event,
                TREASURY_VAULT_LABEL.to_string(),
                holder.to_string(),
                "SOL".to_string(),
                amount,
                "borrow",
            ))
        }
        "FeesCollected" => {
            let operator = pubkey_field(f, "operator")?;
            let amount = u64_field(f, "amount_raw")?;
            Some(transfer(
                program_id,
                event,
                FEE_VAULT_LABEL.to_string(),
                operator.to_string(),
                "SOL".to_string(),
                amount,
                "collect_fees",
            ))
        }
        "WhaleTrancheExecuted" => {
            let holder = pubkey_field(f, "holder")?;
            let amount = u64_field(f, "amount_raw")?;
            Some(transfer(
                program_id,
                event,
                TREASURY_VAULT_LABEL.to_string(),
                holder.to_string(),
                "SOL".to_string(),
                amount,
                "execute_whale_tranche",
            ))
        }
        _ => None,
    }
}
