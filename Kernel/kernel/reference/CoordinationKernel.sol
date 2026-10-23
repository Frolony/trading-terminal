// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

// ============================================================================
// INTERFACES & EVALUATORS
// ============================================================================

interface IConstraint {
    function validate(bytes32 stateId, bytes32 transitionId, bytes32 observationId)
        external view returns (bool pass, uint8 status);
}

interface IObjective {
    function evaluate(bytes32 stateId, bytes32 transitionId, bytes32 observationId)
        external view returns (int256 score, bool satisfied);
}

interface IRelevanceFunction {
    function evaluate(address agent, bytes32 transitionId, bytes32 observationId)
        external view returns (uint256 relevance);
}

interface IPlanCompatibilityEngine {
    function evaluate(bytes32 transitionId, bytes32[] calldata planIds)
        external view returns (
            bool compatible,
            bytes32[] memory conflictingPlans,
            bytes32[] memory conflictingResources,
            bytes32[] memory unresolvedDependencies,
            uint256 coverage
        );
}

interface IAttributionRule {
    function evaluate(bytes32 transitionId, bytes32 outcomeId)
        external view returns (
            address[] memory agents,
            int256[] memory deltaRep,
            bytes32[] memory modelProposalIds
        );
}

interface ICoordinationProtocol {
    struct ProtocolState {
        bytes32 procedureId;
        bytes32 policyHash;
        uint256 quorumRequired;
        uint256 confidenceThreshold;
        uint256 maxRounds;
    }

    function initialize(bytes32 transitionType, bytes32 procedureId, uint256 quorum, uint256 confThresh, uint256 maxRounds) 
        external pure returns (bytes32 protocolStateHash);

    function evaluateSelection(
        bytes32[] calldata candidateTransitionIds,
        bytes32 trustMerkleRoot,
        bytes32 protocolStateHash,
        bytes32 proofOfNegotiationHash
    ) external view returns (bool success, bytes32 selectedTransitionId);
}

// ============================================================================
// KERNEL STORAGE & TYPES (LAYERS 1, 2, 3)
// ============================================================================

abstract contract IntegratedKernelStorage {
    // ------------------------------------------------------------------------
    // Constants
    // ------------------------------------------------------------------------
    uint256 public constant WAD = 1e18;
    uint256 public constant MAX_SET = 32;
    uint256 public constant MAX_ARR = 32;
    uint256 public constant MAX_GROUPS = 16;
    uint256 public constant MAX_PLANS_HARD = 64;
    uint256 public constant MAX_EXCEPTIONS = 64;
    uint256 public constant MAX_VERSIONS = 256;
    uint256 public constant RELEVANCE_CAP = 1e24;
    uint256 public constant MAX_INFLUENCE = 1e30;
    int256 public constant REP_MIN = -1000e18;
    int256 public constant REP_MAX = 1000e18;
    int256 public constant MAX_DELTA = 100e18;
    uint256 public constant NO_VERSION = type(uint256).max;

    // Failure Bits
    uint256 public constant F_CONSTRAINTS = 1;
    uint256 public constant F_OBJECTIVES = 2;
    uint256 public constant F_QUORUM = 4;
    uint256 public constant F_CONFIDENCE = 8;
    uint256 public constant F_COMPAT = 16;
    uint256 public constant F_EXCEPTIONS = 32;
    uint256 public constant F_FRESH = 64;
    /// Renamed from F_DEADLINE (see NotationChange log): the bit means
    /// "planning window is still open", not "a deadline was missed". The
    /// logic is unchanged — commit cannot proceed while planning is open —
    /// only the name was corrected to say what it actually tests.
    uint256 public constant F_PLANNING_OPEN = 128;
    uint256 public constant F_EXPIRED = 256;
    uint256 public constant F_LINEAGE = 512;
    uint256 public constant F_PLANS = 1024;
    uint256 public constant F_CAPABILITY = 2048;
    /// A transition depends on a dialect change that is neither committed nor
    /// still live against the committed dialect (its prevChangeId went stale).
    uint256 public constant F_DIALECT = 4096;
    uint256 public constant MAX_DIALECT_DEPS = 8;

    // ------------------------------------------------------------------------
    // Enums
    // ------------------------------------------------------------------------
    enum SystemState { OPERATIONAL, COLLAPSED_FROZEN, REBOOTSTRAPPING, TERMINATED }
    enum ObservationStatus { NONE, INGESTED, EXPIRED, INVALID }
    enum TransitionLifecycle {
        NONE, PROPOSED, EVALUATED, ELIGIBLE, SELECTED, PLANNING,
        RECONCILING, VALIDATED, COMMITTABLE, COMMITTED, EXECUTING,
        SETTLED, EVALUATED_OUTCOME, DISCARDED, REJECTED, EXPIRED, CONFLICTED, BLOCKED
    }
    enum ExceptionDisposition { INFORMATIONAL, REVIEW, BLOCK, EMERGENCY }
    enum ResolutionKind { UNRESOLVED, REMEDIATED, WAIVED, DEFECTION_CONFIRMED, DEFECTION_DISMISSED }
    enum RegistryKind { CONSTRAINT, OBJECTIVE, RELEVANCE, ATTRIBUTION }

    // ------------------------------------------------------------------------
    // Structs
    // ------------------------------------------------------------------------
    struct State { bytes32 id; uint256 index; bytes32 stateHash; }

    struct ObservationSnapshot {
        bytes32 id;
        uint256 timestamp;
        bytes32 sourceSetHash;
        bytes32 dataHash;
        bytes32 methodologyHash;
        uint256 confidence;
        uint256 expiresAt;
    }

    struct ManifestInput {
        bytes32 parentStateId;
        bytes32 transitionType;
        bytes32[] relevantGroups;
        bytes32[] changedVars;
        bytes32[] constraints;
        bytes32[] resources;
        bytes32[] expectedEffects;
        uint256 expiresAt;
        bytes32 observationId;
        bytes32 resultingStateHash;
        bytes32 transitionPayloadHash;
    }

    struct Manifest {
        bytes32 parentStateId;
        bytes32 transitionType;
        bytes32[] relevantGroups;
        bytes32[] changedVars;
        bytes32[] constraints;
        bytes32[] resources;
        bytes32[] expectedEffects;
        uint256 expiresAt;
        bytes32 observationId;
        bytes32 resultingStateHash;
        bytes32 transitionPayloadHash;
    }

    struct Transition {
        bytes32 id;
        bytes32 parentStateId;
        bytes32 proposedStateId;
        address proposer;
        Manifest manifest;
    }

    struct PlanInput {
        bytes32[] resourceClaims;
        bytes32[] effects;
        bytes32[] prerequisites;
        bytes32[] conflicts;
        uint256 expiresAt;
        bytes32 payloadHash;
    }

    struct Plan {
        bytes32 id;
        address agent;
        bytes32 candidateTransitionId;
        bytes32[] resourceClaims;
        bytes32[] effects;
        bytes32[] prerequisites;
        bytes32[] conflicts;
        uint256 expiresAt;
        bytes32 payloadHash;
    }

    struct Policy {
        bool exists;
        uint256 quorumRequired;
        uint256 confidenceThreshold;
        uint256 coverageMin;
        uint256 minObsConfidence;
        uint256 consensusWindow;
        uint256 planningWindow;
        uint256 executionWindow;
        uint256 maxPlans;
        bool requireObjectives;
        uint256 settlementThreshold;
        uint256 trustUpdateRate; // WAD (Layer 2 -> Layer 3 scaling factor)
        uint256 maxConsensusRounds;
    }

    struct EvaluationResult {
        bool constraintsSatisfied;
        bool objectivesSatisfied;
        bytes32[] constraintIds;
        uint8[] constraintStatus;
        bytes32[] objectiveIds;
        int256[] objectiveScores;
        bool[] objectiveSatisfied;
        uint256 confidence;
        bytes32 observationId;
    }

    struct EvaluationContext {
        bytes32 policyHash;
        uint256[] constraintVersions;
        uint256[] objectiveVersions;
        uint256[] relevanceVersions;
        uint256 engineVersion;
        uint256[] attributionVersions;
    }

    struct ReconciliationResult {
        bool compatible;
        bool engineFailed;
        bytes32[] conflictingPlans;
        bytes32[] conflictingResources;
        bytes32[] unresolvedDependencies;
        uint256 coverage;
    }

    struct ConsensusRoundSignal {
        address agent;
        uint256 roundIndex;
        uint256 rawWeight;
        uint256 effectiveWeight;
        uint256 support;
        uint64 timestamp;
    }

    struct ExceptionSignal {
        bytes32 id;
        address agent;
        address offender;
        uint256 severity;
        ExceptionDisposition disposition;
        bool resolved;
        ResolutionKind resolutionKind;
        bytes32 resolutionEvidenceHash;
    }

    struct CapabilityRequirement {
        bytes32 capabilityId;
        uint256 minWeightRequired;
    }

    struct BranchingHealth {
        uint256 capabilityScore; // WAD
        uint256 trustScore;      // WAD
        uint256 contextScore;    // WAD
        uint256 coverageScore;   // WAD
        uint256 minAggregate;    // min(cap, trust, context, coverage)
    }

    struct Version { address impl; uint64 activatesAt; bool retired; }
    struct RepCheckpoint { uint64 ts; int256 rep; }
    struct AgentProfile { bool active; bytes32[] groups; int256 reputation; uint256 baseInfluence; }

    // --- Dialect (Layer 3): proposals to change what a symbol means ----------
    //
    // A NotationChange is a PROPOSAL, not an effect. It takes effect only when a
    // transition that declared a dependency on it commits (see commit()), so it
    // rides the same coordination process as every other proposal. Until then the
    // old and new definitions coexist as a declared divergence.
    //
    // Pull, not push: nothing on the evaluation path ever reads this registry.
    // Agents doing ordinary work use the committed dialect for free. Only an
    // agent proposing a change pays to read it: it must cite prevChangeId (the
    // committed state it is changing), which makes the proposal legible as a
    // change rather than a silent redefinition in place. The prevChangeId chain
    // is documentation for the proposer; the kernel never walks it.
    enum SymbolKind { CONSTANT, FUNCTION, PREDICATE, TYPE, ROLE }
    enum NotationStatus { NONE, PROPOSED, COMMITTED }

    struct NotationChange {
        bytes32 oldSymbol;
        bytes32 newSymbol;       // == oldSymbol is legal (same name, new kind or meaning)
        SymbolKind oldKind;
        SymbolKind newKind;      // oldKind != newKind with equal symbols = reclassification
        /// keccak256 of the UTF-8 text defining newSymbol's meaning, published off-chain
        /// or in the event log. An agent fetches the text and checks it hashes to this
        /// value; two definitions are the same iff their hashes are equal. Lets a
        /// same-name, same-kind redefinition be told apart from a no-op.
        bytes32 definitionHash;
        bytes32 prevChangeId;    // committed head of oldSymbol when proposed; 0 = original meaning
        string reason;
        address proposer;
        uint64 proposedAt;
        uint64 committedAt;      // 0 until a dependent transition commits
        uint256 committedEpoch;  // dialectEpoch after this change took effect; 0 until committed
        NotationStatus status;
    }

    // ------------------------------------------------------------------------
    // Layer 1 Substrate Storage
    // ------------------------------------------------------------------------
    SystemState public systemState = SystemState.OPERATIONAL;
    bytes32 public constant GOVERNANCE = keccak256("GOVERNANCE");
    bytes32 public constant GUARDIAN = keccak256("GUARDIAN");
    bytes32 public constant OBSERVER = keccak256("OBSERVER");
    bytes32 public constant PROPOSER = keccak256("PROPOSER");
    bytes32 public constant EXECUTOR = keccak256("EXECUTOR");
    bytes32 public constant SETTLER = keccak256("SETTLER");

    mapping(bytes32 => mapping(address => bool)) public hasRole;
    bool public paused;
    uint256 public evaluatorGasCap = 500_000;
    uint256 public registryDelay;

    State public declaredState;
    mapping(bytes32 => State) public realizedStates;
    mapping(bytes32 => uint256) public committedIndex;

    mapping(bytes32 => ObservationSnapshot) public observations;
    mapping(bytes32 => ObservationStatus) public observationStatus;

    mapping(bytes32 => Transition) internal transitions;
    mapping(bytes32 => TransitionLifecycle) public transitionLifecycle;
    mapping(TransitionLifecycle => mapping(TransitionLifecycle => bool)) internal _allowed;

    mapping(bytes32 => Policy) public policies;
    mapping(bytes32 => bytes32[]) public requiredConstraints;
    mapping(bytes32 => bytes32[]) public requiredObjectives;
    mapping(bytes32 => bytes32[]) public requiredRelevance;
    mapping(bytes32 => bytes32[]) public requiredAttribution;

    mapping(bytes32 => Policy) public transitionPolicy;
    mapping(bytes32 => bytes32[]) public constraintSet;
    mapping(bytes32 => bytes32[]) public objectiveSet;
    mapping(bytes32 => bytes32[]) public relevanceSet;
    mapping(bytes32 => bytes32[]) public attributionSet;
    mapping(bytes32 => EvaluationContext) internal context;

    mapping(bytes32 => Version[]) internal _constraintReg;
    mapping(bytes32 => Version[]) internal _objectiveReg;
    mapping(bytes32 => Version[]) internal _relevanceReg;
    mapping(bytes32 => Version[]) internal _attributionReg;
    Version[] internal _engineReg;

    mapping(bytes32 => EvaluationResult) public evaluationResults;
    mapping(bytes32 => ReconciliationResult) public reconciliationResults;

    mapping(address => AgentProfile) public agents;
    mapping(address => RepCheckpoint[]) internal repHistory;

    // Sparse Root for Trust Matrix (Layer 1 single source of truth)
    bytes32 public trustMerkleRoot;

    // Dynamic Relevance Mapping: transitionId => agent => channelId => score
    mapping(bytes32 => mapping(address => mapping(bytes32 => uint256))) public vectorRelevance;

    // ------------------------------------------------------------------------
    // Layer 2 Protocol Storage
    // ------------------------------------------------------------------------
    mapping(bytes32 => uint256) public eligibleAt;
    mapping(bytes32 => uint256) public consensusDeadline;
    mapping(bytes32 => uint256) public currentConsensusRound;
    mapping(bytes32 => mapping(uint256 => mapping(address => ConsensusRoundSignal))) public roundSignals;
    mapping(bytes32 => mapping(uint256 => uint256)) public roundTotalWeight;
    mapping(bytes32 => mapping(uint256 => uint256)) public roundWeightedSupport;
    mapping(bytes32 => bytes32) public selectedForParent;

    mapping(bytes32 => ExceptionSignal[]) public exceptionSignals;
    mapping(bytes32 => uint256) public unresolvedBlocking;

    mapping(bytes32 => Plan) public plans;
    mapping(bytes32 => bytes32[]) public transitionPlans;
    mapping(bytes32 => uint256) public planningDeadline;
    mapping(bytes32 => uint256) public minPlanExpiry;

    mapping(bytes32 => CapabilityRequirement[]) public requiredCapabilities;
    mapping(address => mapping(bytes32 => bool)) public agentCapabilities;

    // ------------------------------------------------------------------------
    // Layer 3 Evolutionary Storage
    // ------------------------------------------------------------------------
    mapping(uint256 => BranchingHealth) public stateHealthHistory;
    mapping(bytes32 => uint256) public transitionConsensusFailures;

    // ------------------------------------------------------------------------
    // Dialect registry (see NotationChange). No enumeration arrays: proposers
    // walk prevChangeId links; indexers follow events.
    // ------------------------------------------------------------------------
    mapping(bytes32 => NotationChange) public notationChanges;
    /// symbol -> id of the last COMMITTED change that touched it (old or new
    /// name); bytes32(0) means the symbol still has its original meaning.
    mapping(bytes32 => bytes32) public dialectHead;
    /// Bumped once per committed change. Context(T) can record the epoch it was
    /// registered under; an evaluator from an earlier epoch still functions, its
    /// context is simply a previous epoch's.
    uint256 public dialectEpoch;
    uint256 internal notationNonce;
    mapping(bytes32 => bytes32[]) public dialectDeps; // transitionId -> change ids

    // ------------------------------------------------------------------------
    // Errors & Events
    // ------------------------------------------------------------------------
    error Unauthorized();
    error SystemCollapsed();
    error IsPaused();
    error BadLifecycle(TransitionLifecycle have);
    error CommitBlocked(uint256 failMask);
    error AlreadySignalled();
    error NotEligible();
    error MonotoneRevisionViolation();
    error ConsensusFailedMaxRounds();
    error RaiserCannotResolveOwnBlockingException();
    error BadLifecycleTransition(TransitionLifecycle from, TransitionLifecycle to);
    error AlreadySignalledThisRound();
    error InactiveAgent();
    error EmptySymbol();
    error StalePrevChange(bytes32 expectedHead, bytes32 given);
    error UnknownNotationChange();
    error NotTransitionProposer();
    error DialectDepCap();
    error DialectDepConflict();
    error NotPreCommit();
    error PriorKindMismatch(SymbolKind committedKind, SymbolKind cited);
    error NoOpChange();

    event NotationProposed(bytes32 indexed id, bytes32 indexed oldSymbol, bytes32 indexed newSymbol, NotationChange change);
    event NotationCommitted(bytes32 indexed id, bytes32 indexed transitionId, uint256 epoch);
    event DialectDependencyDeclared(bytes32 indexed transitionId, bytes32 indexed changeId);
    event SystemStateChanged(SystemState indexed from, SystemState indexed to);
    event TrustMerkleRootUpdated(bytes32 indexed newRoot);
    event SignalRevised(bytes32 indexed transitionId, uint256 indexed round, address indexed agent, uint256 support);
    event ConsensusFailed(bytes32 indexed transitionId, uint256 roundsExecuted);
    event ExceptionSignalSubmitted(bytes32 indexed transitionId, address indexed agent, uint256 index);
    event ExceptionResolved(bytes32 indexed transitionId, uint256 index, ResolutionKind kind, bytes32 evidenceHash);
    event BranchingHealthUpdated(uint256 indexed stateIndex, uint256 minAggregate);
    event BranchingCollapseDetected(uint256 indexed stateIndex);
}

// ============================================================================
// CORE COORDINATION KERNEL (LAYERS 1, 2, 3 UNIFIED)
// ============================================================================

contract CoordinationKernel is IntegratedKernelStorage {

    constructor(address governance, bytes32 genesisStateId, bytes32 genesisStateHash) {
        hasRole[GOVERNANCE][governance] = true;
        declaredState = State(genesisStateId, 0, genesisStateHash);

        _allow(TransitionLifecycle.PROPOSED, TransitionLifecycle.EVALUATED);
        _allow(TransitionLifecycle.PROPOSED, TransitionLifecycle.REJECTED);
        _allow(TransitionLifecycle.EVALUATED, TransitionLifecycle.ELIGIBLE);
        _allow(TransitionLifecycle.ELIGIBLE, TransitionLifecycle.SELECTED);
        _allow(TransitionLifecycle.SELECTED, TransitionLifecycle.PLANNING);
        _allow(TransitionLifecycle.PLANNING, TransitionLifecycle.RECONCILING);
        _allow(TransitionLifecycle.RECONCILING, TransitionLifecycle.VALIDATED);
        _allow(TransitionLifecycle.RECONCILING, TransitionLifecycle.BLOCKED);
        _allow(TransitionLifecycle.VALIDATED, TransitionLifecycle.COMMITTABLE);
        _allow(TransitionLifecycle.VALIDATED, TransitionLifecycle.BLOCKED);
        _allow(TransitionLifecycle.COMMITTABLE, TransitionLifecycle.COMMITTED);
        _allow(TransitionLifecycle.COMMITTED, TransitionLifecycle.EXECUTING);
        _allow(TransitionLifecycle.EXECUTING, TransitionLifecycle.SETTLED);
        _allow(TransitionLifecycle.SETTLED, TransitionLifecycle.EVALUATED_OUTCOME);
        _allow(TransitionLifecycle.BLOCKED, TransitionLifecycle.VALIDATED);
        _allow(TransitionLifecycle.BLOCKED, TransitionLifecycle.PLANNING);

        // Genesis dialect: changes already baked into this deployment are
        // recorded as COMMITTED at construction (there is no earlier chain to
        // cite, so prevChangeId = 0), so the history is discoverable on-chain.
        _genesisNotation(
            keccak256("F_DEADLINE"), keccak256("F_PLANNING_OPEN"),
            SymbolKind.CONSTANT, SymbolKind.CONSTANT,
            "bit tests whether the planning window is still open, not whether a deadline was missed; logic unchanged"
        );
        _genesisNotation(
            keccak256("declareNotationChange"), keccak256("proposeNotationChange"),
            SymbolKind.FUNCTION, SymbolKind.FUNCTION,
            "governance-only log with immediate effect became an agent proposal that takes effect only when a dependent transition commits"
        );
    }

    function _allow(TransitionLifecycle a, TransitionLifecycle b) private {
        _allowed[a][b] = true;
    }

    /// Guarded lifecycle move: reverts if `cur -> next` was never declared
    /// allowed in the constructor's transition table. Replaces direct
    /// `transitionLifecycle[id] = X` assignments, which previously let any
    /// function silently overwrite lifecycle state (including regressing a
    /// COMMITTED transition) with no check at all.
    function _set(bytes32 id, TransitionLifecycle next) internal {
        TransitionLifecycle cur = transitionLifecycle[id];
        if (!_allowed[cur][next]) revert BadLifecycleTransition(cur, next);
        transitionLifecycle[id] = next;
    }

    function _require(bytes32 id, TransitionLifecycle want) internal view {
        TransitionLifecycle cur = transitionLifecycle[id];
        if (cur != want) revert BadLifecycle(cur);
    }

    // ------------------------------------------------------------------------
    // Dialect proposals (Layer 3 event; see NotationChange)
    // ------------------------------------------------------------------------

    /// Propose that `oldSymbol` (of kind oldKind) become `newSymbol` (of kind
    /// newKind). Any active agent may propose. The proposer must cite
    /// `prevChangeId` = dialectHead[oldSymbol], the committed state being changed;
    /// a stale citation reverts. Several proposals may cite the same prev: they
    /// coexist as declared divergence until one commits, which makes the rest stale.
    /// Never gates ordinary calls, and takes no effect by itself.
    struct NotationInput {
        bytes32 oldSymbol;
        bytes32 newSymbol;
        SymbolKind oldKind;
        SymbolKind newKind;
        bytes32 definitionHash;
        bytes32 prevChangeId;
        string reason;
    }

    function proposeNotationChange(NotationInput calldata p) external returns (bytes32 id) {
        if (paused) revert IsPaused();
        if (!agents[msg.sender].active) revert InactiveAgent();
        if (p.oldSymbol == bytes32(0) || p.newSymbol == bytes32(0)) revert EmptySymbol();
        bytes32 head = dialectHead[p.oldSymbol];
        if (p.prevChangeId != head) revert StalePrevChange(head, p.prevChangeId);
        _checkLegibleChange(p, head);

        id = keccak256(abi.encode(msg.sender, p.oldSymbol, p.newSymbol, p.prevChangeId, ++notationNonce));
        notationChanges[id] = NotationChange({
            oldSymbol: p.oldSymbol,
            newSymbol: p.newSymbol,
            oldKind: p.oldKind,
            newKind: p.newKind,
            definitionHash: p.definitionHash,
            prevChangeId: p.prevChangeId,
            reason: p.reason,
            proposer: msg.sender,
            proposedAt: uint64(block.timestamp),
            committedAt: 0,
            committedEpoch: 0,
            status: NotationStatus.PROPOSED
        });
        emit NotationProposed(id, p.oldSymbol, p.newSymbol, notationChanges[id]);
    }

    /// The cited prior state must actually be the one being changed, and the
    /// proposal must change something. Compared against the committed head only:
    /// a symbol whose head is 0 keeps its original meaning, which predates the
    /// chain, so there is nothing recorded to compare with.
    function _checkLegibleChange(NotationInput calldata p, bytes32 head) internal view {
        if (head == bytes32(0)) return;
        NotationChange storage h = notationChanges[head];
        if (h.newSymbol != p.oldSymbol) return; // head retired this name; redefining it starts fresh
        if (p.oldKind != h.newKind) revert PriorKindMismatch(h.newKind, p.oldKind);
        if (p.newSymbol == p.oldSymbol && p.newKind == h.newKind && p.definitionHash == h.definitionHash) {
            revert NoOpChange();
        }
    }

    /// A transition's proposer declares that the transition depends on a dialect
    /// change; the change takes effect if and when the transition commits.
    function declareDialectDependency(bytes32 transitionId, bytes32 changeId) external {
        Transition storage t = transitions[transitionId];
        if (t.proposer == address(0) || t.proposer != msg.sender) revert NotTransitionProposer();
        TransitionLifecycle cur = transitionLifecycle[transitionId];
        if (!((uint8(cur) >= 1 && uint8(cur) <= 8) || cur == TransitionLifecycle.BLOCKED)) revert NotPreCommit();
        NotationChange storage c = notationChanges[changeId];
        if (c.status != NotationStatus.PROPOSED) revert UnknownNotationChange();

        bytes32[] storage deps = dialectDeps[transitionId];
        if (deps.length >= MAX_DIALECT_DEPS) revert DialectDepCap();
        // Every change moves dialectHead for both of its names, so two dependencies
        // of one transition may not share ANY symbol (old or new). This covers same
        // old name, two names renamed to one, and chains like X->Y plus Y->Z, any of
        // which would pass the predicate yet go stale part-way through commit().
        for (uint256 i = 0; i < deps.length; i++) {
            NotationChange storage d = notationChanges[deps[i]];
            if (
                d.oldSymbol == c.oldSymbol || d.oldSymbol == c.newSymbol ||
                d.newSymbol == c.oldSymbol || d.newSymbol == c.newSymbol
            ) revert DialectDepConflict();
        }
        deps.push(changeId);
        emit DialectDependencyDeclared(transitionId, changeId);
    }

    /// True iff every dialect change the transition depends on is committed, or is
    /// still live: proposed against the dialect that is currently committed.
    function dialectDepsOk(bytes32 id) public view returns (bool) {
        bytes32[] storage deps = dialectDeps[id];
        for (uint256 i = 0; i < deps.length; i++) {
            NotationChange storage c = notationChanges[deps[i]];
            if (c.status == NotationStatus.COMMITTED) continue;
            if (c.status != NotationStatus.PROPOSED || c.prevChangeId != dialectHead[c.oldSymbol]) return false;
        }
        return true;
    }

    /// Runs inside commit(), after the predicate passed: dependent changes take
    /// effect now, atomically with the state transition that relies on them.
    function _commitDialectDeps(bytes32 transitionId) internal {
        bytes32[] storage deps = dialectDeps[transitionId];
        for (uint256 i = 0; i < deps.length; i++) {
            NotationChange storage c = notationChanges[deps[i]];
            if (c.status == NotationStatus.COMMITTED) continue;
            bytes32 head = dialectHead[c.oldSymbol];
            if (c.prevChangeId != head) revert StalePrevChange(head, c.prevChangeId);
            c.status = NotationStatus.COMMITTED;
            c.committedAt = uint64(block.timestamp);
            c.committedEpoch = ++dialectEpoch;
            dialectHead[c.oldSymbol] = deps[i];
            dialectHead[c.newSymbol] = deps[i];
            emit NotationCommitted(deps[i], transitionId, c.committedEpoch);
        }
    }

    function _genesisNotation(
        bytes32 oldSymbol, bytes32 newSymbol, SymbolKind oldKind, SymbolKind newKind, string memory reason
    ) private {
        bytes32 id = keccak256(abi.encode(msg.sender, oldSymbol, newSymbol, bytes32(0), ++notationNonce));
        notationChanges[id] = NotationChange({
            oldSymbol: oldSymbol,
            newSymbol: newSymbol,
            oldKind: oldKind,
            newKind: newKind,
            definitionHash: keccak256(bytes(reason)),
            prevChangeId: dialectHead[oldSymbol],
            reason: reason,
            proposer: msg.sender,
            proposedAt: uint64(block.timestamp),
            committedAt: uint64(block.timestamp),
            committedEpoch: ++dialectEpoch,
            status: NotationStatus.COMMITTED
        });
        dialectHead[oldSymbol] = id;
        dialectHead[newSymbol] = id;
        emit NotationProposed(id, oldSymbol, newSymbol, notationChanges[id]);
        emit NotationCommitted(id, bytes32(0), dialectEpoch);
    }

    modifier onlyRole(bytes32 role) {
        if (!hasRole[role][msg.sender]) revert Unauthorized();
        _;
    }

    modifier whenOperational() {
        if (paused) revert IsPaused();
        if (systemState != SystemState.OPERATIONAL) revert SystemCollapsed();
        _;
    }

    // ------------------------------------------------------------------------
    // Layer 1: Substrate Operations
    // ------------------------------------------------------------------------

    function setTrustMerkleRoot(bytes32 newRoot) external onlyRole(GOVERNANCE) {
        trustMerkleRoot = newRoot;
        emit TrustMerkleRootUpdated(newRoot);
    }

    function verifyTrustProof(
        address source,
        address target,
        uint256 trustScore,
        bytes32[] calldata merkleProof
    ) public view returns (bool) {
        bytes32 leaf = keccak256(abi.encodePacked(source, target, trustScore));
        bytes32 computedHash = leaf;
        for (uint256 i = 0; i < merkleProof.length; i++) {
            bytes32 proofElement = merkleProof[i];
            if (computedHash <= proofElement) {
                computedHash = keccak256(abi.encodePacked(computedHash, proofElement));
            } else {
                computedHash = keccak256(abi.encodePacked(proofElement, computedHash));
            }
        }
        return computedHash == trustMerkleRoot;
    }

    function commitFailMask(bytes32 id) public view returns (uint256 m) {
        Transition storage t = transitions[id];
        Policy storage pol = transitionPolicy[id];
        EvaluationResult storage er = evaluationResults[id];

        if (!er.constraintsSatisfied) m |= F_CONSTRAINTS;
        if (pol.requireObjectives && !er.objectivesSatisfied) m |= F_OBJECTIVES;
        if (roundTotalWeight[id][currentConsensusRound[id]] < pol.quorumRequired) m |= F_QUORUM;
        if (confidenceOf(id) < pol.confidenceThreshold) m |= F_CONFIDENCE;
        if (!reconciliationResults[id].compatible) m |= F_COMPAT;
        if (unresolvedBlocking[id] != 0) m |= F_EXCEPTIONS;
        if (block.timestamp < planningDeadline[id]) m |= F_PLANNING_OPEN;
        if (block.timestamp >= t.manifest.expiresAt) m |= F_EXPIRED;
        if (t.manifest.parentStateId != declaredState.id) m |= F_LINEAGE;
        if (minPlanExpiry[id] == 0 || block.timestamp >= minPlanExpiry[id]) m |= F_PLANS;
        if (!capabilityCovered(id)) m |= F_CAPABILITY;
        if (!dialectDepsOk(id)) m |= F_DIALECT;
    }

    /// NOTE on this fix: this draft has no registerTransition/evaluateTransition/
    /// declareCommit yet (see the Layer-2 section — only commitFailMask, commit,
    /// submitRoundSignal, capabilityCovered, resolveException are implemented),
    /// so nothing currently drives a transition to COMMITTABLE. Guarding commit()
    /// correctly therefore makes it uncallable until that lifecycle is ported
    /// over from TransitionKernel.sol — which is the honest state of this file,
    /// not a new bug. The previous version silently wrote COMMITTED from ANY
    /// prior state (including re-writing an already-COMMITTED transition),
    /// which is the actual bug being closed here.
    function commit(bytes32 id) external whenOperational {
        _require(id, TransitionLifecycle.COMMITTABLE);
        uint256 m = commitFailMask(id);
        if (m != 0) revert CommitBlocked(m);

        Transition storage t = transitions[id];
        declaredState = State({
            id: t.proposedStateId,
            index: declaredState.index + 1,
            stateHash: t.manifest.resultingStateHash
        });
        committedIndex[id] = declaredState.index;

        _set(id, TransitionLifecycle.COMMITTED);
        _commitDialectDeps(id); // dialect changes take effect with the commit that relies on them
    }

    // ------------------------------------------------------------------------
    // Layer 2: Procedural Operations (Consensus, Capabilities, Exceptions)
    // ------------------------------------------------------------------------

    function submitRoundSignal(
        bytes32 id, 
        uint256 rawWeight, 
        uint256 support
    ) external whenOperational {
        uint256 round = currentConsensusRound[id];
        Policy storage pol = transitionPolicy[id];
        if (round >= pol.maxConsensusRounds) revert ConsensusFailedMaxRounds();

        (bool eligible, uint256 mult) = _eligible(msg.sender, id);
        if (!eligible) revert NotEligible();

        // One signal per (transition, round, agent). Without this, a second
        // call in the same round added its weight again on top of the first
        // (roundTotalWeight[id][round] += effWeight unconditionally), letting
        // an agent inflate both quorum and confidence by resubmitting. Signal
        // revision belongs in the NEXT round, which is exactly what the
        // monotone-revision check right below this already assumes.
        if (roundSignals[id][round][msg.sender].timestamp != 0) revert AlreadySignalledThisRound();

        // Monotone Revision Constraint Enforcement:
        // Signal revisions MUST move toward the previous round's mean support
        if (round > 0) {
            uint256 prevMean = roundWeightedSupport[id][round - 1] * WAD / roundTotalWeight[id][round - 1];
            uint256 prevSupport = roundSignals[id][round - 1][msg.sender].support;
            if (prevSupport != 0) {
                uint256 prevDiff = prevSupport > prevMean ? prevSupport - prevMean : prevMean - prevSupport;
                uint256 currDiff = support > prevMean ? support - prevMean : prevMean - support;
                if (currDiff > prevDiff) revert MonotoneRevisionViolation();
            }
        }

        uint256 effWeight = rawWeight * mult / WAD;
        roundSignals[id][round][msg.sender] = ConsensusRoundSignal({
            agent: msg.sender,
            roundIndex: round,
            rawWeight: rawWeight,
            effectiveWeight: effWeight,
            support: support,
            timestamp: uint64(block.timestamp)
        });

        roundTotalWeight[id][round] += effWeight;
        roundWeightedSupport[id][round] += effWeight * support / WAD;

        emit SignalRevised(id, round, msg.sender, support);
    }

    function capabilityCovered(bytes32 id) public view returns (bool) {
        bytes32 transitionType = transitions[id].manifest.transitionType;
        CapabilityRequirement[] storage reqs = requiredCapabilities[transitionType];

        for (uint256 i = 0; i < reqs.length; i++) {
            bytes32 capId = reqs[i].capabilityId;
            uint256 accumWeight = 0;
            bytes32[] storage pIds = transitionPlans[id];
            
            for (uint256 j = 0; j < pIds.length; j++) {
                address planAgent = plans[pIds[j]].agent;
                if (agentCapabilities[planAgent][capId]) {
                    accumWeight += agents[planAgent].baseInfluence;
                }
            }
            if (accumWeight < reqs[i].minWeightRequired) return false;
        }
        return true;
    }

    function resolveException(
        bytes32 id, 
        uint256 index, 
        ResolutionKind kind, 
        bytes32 evidenceHash
    ) external onlyRole(GOVERNANCE) {
        ExceptionSignal storage e = exceptionSignals[id][index];
        bool blocking = e.disposition == ExceptionDisposition.BLOCK || e.disposition == ExceptionDisposition.EMERGENCY;
        if (blocking) {
            if (e.agent == msg.sender) revert RaiserCannotResolveOwnBlockingException();
            unresolvedBlocking[id] -= 1;
        }
        e.resolved = true;
        e.resolutionKind = kind;
        e.resolutionEvidenceHash = evidenceHash;

        emit ExceptionResolved(id, index, kind, evidenceHash);
    }

    function confidenceOf(bytes32 id) public view returns (uint256) {
        uint256 round = currentConsensusRound[id];
        uint256 total = roundTotalWeight[id][round];
        if (total == 0) return 0;
        return roundWeightedSupport[id][round] * WAD / total;
    }

    // ------------------------------------------------------------------------
    // Layer 3: Evolutionary Operations (Health & Collapse Consumption)
    // ------------------------------------------------------------------------

    /// Pure view function: computes multi-dimensional health vector
    function computeBranchingHealth(
        uint256 capScore, 
        uint256 trustScore, 
        uint256 contextScore, 
        uint256 coverageScore
    ) public pure returns (BranchingHealth memory h) {
        h.capabilityScore = capScore;
        h.trustScore = trustScore;
        h.contextScore = contextScore;
        h.coverageScore = coverageScore;

        uint256 minVal = capScore < trustScore ? capScore : trustScore;
        minVal = minVal < contextScore ? minVal : contextScore;
        minVal = minVal < coverageScore ? minVal : coverageScore;
        h.minAggregate = minVal;
    }

    /// State-changing function: records health and triggers Layer 1 collapse if min == 0
    function recordBranchingHealth(
        uint256 stateIndex,
        uint256 capScore, 
        uint256 trustScore, 
        uint256 contextScore, 
        uint256 coverageScore
    ) external onlyRole(GOVERNANCE) returns (BranchingHealth memory h) {
        h = computeBranchingHealth(capScore, trustScore, contextScore, coverageScore);
        stateHealthHistory[stateIndex] = h;

        emit BranchingHealthUpdated(stateIndex, h.minAggregate);

        // Consume Collapse: Shift SystemState to FROZEN if H_min == 0
        if (h.minAggregate == 0 && systemState == SystemState.OPERATIONAL) {
            systemState = SystemState.COLLAPSED_FROZEN;
            emit SystemStateChanged(SystemState.OPERATIONAL, SystemState.COLLAPSED_FROZEN);
            emit BranchingCollapseDetected(stateIndex);
        }
    }

    function rebootstrapSystem() external onlyRole(GOVERNANCE) {
        require(systemState == SystemState.COLLAPSED_FROZEN, "Not Frozen");
        systemState = SystemState.OPERATIONAL;
        emit SystemStateChanged(SystemState.COLLAPSED_FROZEN, SystemState.OPERATIONAL);
    }

    // ------------------------------------------------------------------------
    // Internal Helpers
    // ------------------------------------------------------------------------

    function _eligible(address agent, bytes32 id) internal view returns (bool ok, uint256 mult) {
        AgentProfile storage ap = agents[agent];
        if (!ap.active) return (false, 0);
        Manifest storage mf = transitions[id].manifest;
        bytes32[] storage rg = mf.relevantGroups;
        for (uint256 i = 0; i < ap.groups.length; i++) {
            for (uint256 j = 0; j < rg.length; j++) {
                if (ap.groups[i] == rg[j]) {
                    return (true, WAD);
                }
            }
        }
    }
}