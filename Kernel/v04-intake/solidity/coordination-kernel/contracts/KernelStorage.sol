// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./IEvaluators.sol";

/// Shared layout, types, events, errors and internal helpers.
/// The kernel and every module inherit this, so delegatecalled modules see
/// identical storage. Nothing in here is callable on its own.
abstract contract KernelStorage {
    // ------------------------------------------------------------------------
    // Constants
    // ------------------------------------------------------------------------
    uint256 public constant WAD = 1e18;
    uint256 public constant MAX_SET = 32;          // constraints/objectives per transition
    uint256 public constant MAX_ARR = 32;          // generic array cap
    uint256 public constant MAX_GROUPS = 16;
    uint256 public constant MAX_PLANS_HARD = 64;
    uint256 public constant MAX_EXCEPTIONS = 64;
    uint256 public constant MAX_ATTR_RULES = 8;
    uint256 public constant MAX_ATTR_AGENTS = 32;
    uint256 public constant MAX_VERSIONS = 256;
    uint256 public constant MAX_SIBLINGS = 16;     // eligible transitions competing for one parent
    uint256 public constant MAX_REPLANS = 3;
    uint256 public constant MAX_DIALECT_DEPS = 8;
    uint256 public constant MAX_SEATS = 15;
    uint256 public constant RELEVANCE_CAP = 1e24;
    uint256 public constant MAX_INFLUENCE = 1e30;
    uint256 public constant MAX_GROUP_MULT = 1e20;
    int256 public constant REP_MIN = -1000e18;
    int256 public constant REP_MAX = 1000e18;
    int256 public constant MAX_DELTA = 100e18;     // per agent per outcome
    uint256 public constant NO_VERSION = type(uint256).max;

    // Council rules fixed by the constitution text (the numbers Jeffrey set).
    uint256 public constant MIN_SITTING_EPOCHS = 3;   // a council sits at least this many epochs
    uint256 public constant POSITION_EPOCHS = 3;      // candidacy: position held this many epochs
    uint256 public constant DEFECTION_COOLDOWN_EPOCHS = 3;

    // Emergency brake: bounded so it can never deadlock governance.
    uint256 public constant PAUSE_MAX = 3 days;

    // Classifier
    uint256 public constant UNCERTAIN_SUPPORT = type(uint256).max;
    uint256 public constant NEUTRAL_BAND_LOW = 4 * WAD / 10;   // 0.40
    uint256 public constant NEUTRAL_BAND_HIGH = 6 * WAD / 10;  // 0.60

    uint8 internal constant ST_UNAVAILABLE = 0;
    uint8 internal constant ST_PASS = 1;
    uint8 internal constant ST_FAIL = 2;

    // Commit-predicate failure bits. One predicate, one bitset, 13 terms.
    uint256 public constant F_CONSTRAINTS = 1;
    uint256 public constant F_OBJECTIVES = 2;
    uint256 public constant F_QUORUM = 4;
    uint256 public constant F_CONFIDENCE = 8;
    uint256 public constant F_COMPAT = 16;
    uint256 public constant F_EXCEPTIONS = 32;
    uint256 public constant F_FRESH = 64;
    uint256 public constant F_PLANNING_OPEN = 128;   // was F_DEADLINE; see genesis notation
    uint256 public constant F_EXPIRED = 256;
    uint256 public constant F_LINEAGE = 512;
    uint256 public constant F_PLANS = 1024;
    uint256 public constant F_CAPABILITY = 2048;
    uint256 public constant F_DIALECT = 4096;

    // Governance transition types. A governance change is a transition of one
    // of these types whose payload is the exact call to make (see commitGovernance).
    bytes32 public constant T_GOV_ADJUSTABLE = keccak256("GOV_ADJUSTABLE");
    bytes32 public constant T_GOV_CONSTITUTION = keccak256("GOV_CONSTITUTION");
    bytes32 public constant T_GOV_COUNCIL = keccak256("GOV_COUNCIL");
    bytes32 public constant T_GOV_REBOOT = keccak256("GOV_REBOOT");

    // ------------------------------------------------------------------------
    // Errors
    // ------------------------------------------------------------------------
    error Unauthorized(bytes32 role);
    error IsPaused();
    error PauseCooldown();
    error SystemFrozen();
    error NotFrozen();
    error BadLifecycle(TransitionLifecycle have);
    error IllegalMove(TransitionLifecycle from, TransitionLifecycle to);
    error CommitBlocked(uint256 failMask);
    error UnknownSelector();
    error AgentExists();
    error AlreadyAttested();
    error AlreadyExpired();
    error AlreadyResolved();
    error AlreadySignalledThisRound();
    error ArraysTooLarge();
    error BadConfidence();
    error BadGroups();
    error BadMaxPlans();
    error BadMultiplier();
    error BadObservationWindow();
    error BadSettlementThreshold();
    error BadRounds();
    error CommitWindowClosed();
    error ConfidenceBelowThreshold();
    error ConsensusClosed();
    error ConsensusStillOpen();
    error AlreadyRecorded();
    error ConstraintsNotSatisfied();
    error ExceptionCap();
    error ExceptionsNotAccepted();
    error ExecutionWindowClosed();
    error GasCapRange();
    error GoalDimensionMismatch();
    error GoalExists();
    error InactiveAgent();
    error ManifestTooLarge();
    error MonotoneRevisionViolation();
    error NoActiveCompatibilityEngine();
    error NoPolicyForType();
    error NotAContract();
    error NotEligible();
    error NotExpired();
    error NotGuardian();
    error NotIngested();
    error NotPreCommit();
    error NotTransitionProposer();
    error ObjectivesNotSatisfied();
    error ObsExists();
    error ObsNotIngested();
    error ObservationNotFresh();
    error ObservationNotIngested();
    error OutOfBounds();
    error OutcomeExists();
    error OutrankedBySibling(bytes32 sibling);
    error SiblingsStillOpen(bytes32 sibling);
    error SiblingCap();
    error ParentAlreadyHasASelectedTransition();
    error ParentNeDeclared();
    error ParentStillCurrent();
    error PlanCap();
    error PlanExpiresTooEarly();
    error PlanTooLarge();
    error PlanningClosed();
    error PlanningWindowOpen();
    error QuorumNotMet();
    error RaiserCannotResolveOwnBlockingException();
    error ReplanCap();
    error ScenarioExists();
    error SetGtMAXSET();
    error SetTooLarge();
    error StillBlocked();
    error TooManyVersions();
    error TransitionExists();
    error TransitionExpired();
    error UnknownAgent();
    error ValueOutOfRange();
    error ViolatedConstraintNotInSet();
    error WadOutOfRange();
    error ZeroGovernance();
    // governance / council
    error NotCouncil();
    error CouncilNotUnanimous(address missing);
    error NotGenesis();
    error GenesisIsSealed();
    error GenesisNotReady(bytes32 what);
    error UnknownGovAction(bytes4 selector);
    error WrongGovCategory(bytes32 have, bytes32 want);
    error GovernanceTypeOnly();       // a governance type cannot be registered as an ordinary transition
    error UseGovernanceCommit();      // governance transitions commit only through commitGovernance
    error NotGovernanceTransition();
    error GovActionMismatch();
    error GovActionFailed(bytes reason);
    error BadSeats();
    error SittingTooShort(uint256 sittingSince, uint256 epoch);
    error NotCandidate(address agent);
    error NoPositionSource();
    error AlreadyVoted();
    error InfluenceShareExceeded();
    error BadEpochLength();
    // dialect
    error EmptySymbol();
    error StalePrevChange(bytes32 expectedHead, bytes32 given);
    error UnknownNotationChange();
    error DialectDepCap();
    error DialectDepConflict();
    error PriorKindMismatch(SymbolKind committedKind, SymbolKind cited);
    error NoOpChange();

    // ------------------------------------------------------------------------
    // Enums
    // ------------------------------------------------------------------------
    enum SystemState { OPERATIONAL, COLLAPSED_FROZEN, REBOOTSTRAPPING, TERMINATED }
    enum ObservationStatus { NONE, INGESTED, EXPIRED, INVALID }

    enum TransitionLifecycle {
        NONE,               // 0
        PROPOSED,           // 1
        EVALUATED,          // 2
        ELIGIBLE,           // 3
        SELECTED,           // 4
        PLANNING,           // 5
        RECONCILING,        // 6
        VALIDATED,          // 7
        COMMITTABLE,        // 8
        COMMITTED,          // 9
        EXECUTING,          // 10
        SETTLED,            // 11
        EVALUATED_OUTCOME,  // 12
        DISCARDED,          // 13
        REJECTED,           // 14
        EXPIRED,            // 15
        CONFLICTED,         // 16
        BLOCKED,            // 17
        COMPENSATING        // 18 (reserved, not wired)
    }

    enum ExceptionDisposition { INFORMATIONAL, REVIEW, BLOCK, EMERGENCY }
    enum ResolutionKind { UNRESOLVED, REMEDIATED, WAIVED, DEFECTION_CONFIRMED, DEFECTION_DISMISSED }
    enum RegistryKind { CONSTRAINT, OBJECTIVE, RELEVANCE, ATTRIBUTION }

    /// Stance: position read from a round signal. NEUTRAL is a declared middle
    /// position; UNCERTAIN is "no position" (the UNCERTAIN_SUPPORT sentinel).
    enum Stance { ALIGNED, OPPOSED, NEUTRAL, UNCERTAIN }
    /// Contribution: the KIND of participation. Derived by the kernel from facts
    /// it already holds (never self-declared, never set by a trusted party).
    enum Contribution { SUBSTANTIVE, PROCEDURAL, INFORMATIONAL, OBSTRUCTIVE, ABSTAINING, SPECIALIST }
    enum Sector { ALIGNED, MISALIGNED, COLLABORATIVE, DIVERGENT, ADVERSARIAL, SPECIALIST, DISASSOCIATED, ABSTAINING, UNCERTAIN }

    enum SymbolKind { CONSTANT, FUNCTION, PREDICATE, TYPE, ROLE }
    enum NotationStatus { NONE, PROPOSED, COMMITTED }

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
        uint256 confidence; // WAD
        uint256 expiresAt;
    }

    struct Scenario { bytes32 id; bytes32 observationId; bytes32 stateHash; uint256 confidence; }

    struct Goal {
        bytes32 id;
        bytes32 stateHash;
        bytes32[] objectiveIds;
        int256[] objectiveVector;
    }

    struct ManifestInput {
        bytes32 parentStateId;
        bytes32 transitionType;
        bytes32[] relevantGroups;
        bytes32[] changedVars;
        bytes32[] constraints;      // proposer-supplied ADDITIONS to the mandatory set
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

    struct PlanInput {
        bytes32[] resourceClaims;
        bytes32[] effects;
        bytes32[] prerequisites;
        bytes32[] conflicts;
        uint256 expiresAt;
        bytes32 payloadHash;
    }

    /// Per-transition-type policy. Registered only through a committed governance
    /// transition; copied into each transition at registration (frozen).
    struct Policy {
        bool exists;
        uint256 quorumRequired;       // total effective weight
        uint256 confidenceThreshold;  // WAD
        uint256 coverageMin;          // engine coverage units
        uint256 minObsConfidence;     // WAD
        uint256 consensusWindow;      // seconds, split evenly across the rounds
        uint256 planningWindow;       // seconds
        uint256 executionWindow;      // seconds (for governance types: the apply window is atomic)
        uint256 maxPlans;
        bool requireObjectives;
        uint256 settlementThreshold;  // distinct SETTLER attestations
        uint256 maxConsensusRounds;   // >= 1
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

    /// Immutable-once-written binding of the exact rule versions used.
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

    struct CommitConditions {
        uint256 quorumRequired;
        uint256 quorumAchieved;
        uint256 planningDeadline;
        uint256 executionDeadline;
        uint256 expiresAt;
    }

    struct Outcome {
        bytes32 id;
        bytes32 transitionId;
        bytes32 resultingStateId;
        bool successful;
        bytes32[] observedEffects;
        bytes32[] violatedConstraints;
        bytes32[] unexpectedEffects;
        bytes32 correctiveTransitionId;
    }

    struct AgentProfile {
        bool active;
        bytes32[] groups;
        int256 reputation;
        uint256 baseInfluence;
    }

    struct RepCheckpoint { uint64 ts; int256 rep; }

    struct ConsensusSignal {
        address agent;
        uint256 rawWeight;
        uint256 effectiveWeight;
        uint256 support; // WAD, or UNCERTAIN_SUPPORT
        uint64 at;
    }

    struct Tally { uint256 totalWeight; uint256 weightedSupport; uint256 count; }

    struct ExceptionSignal {
        bytes32 id;
        address agent;       // raiser
        address offender;    // who the exception is about (0 = nobody in particular)
        uint256 severity;
        ExceptionDisposition disposition;
        bool resolved;
        ResolutionKind resolutionKind;
        bytes32 resolutionEvidenceHash;
    }

    struct CapabilityRequirement { bytes32 capabilityId; uint256 minWeightRequired; }

    struct BranchingHealth {
        uint256 capabilityScore;
        uint256 trustScore;
        uint256 contextScore;
        uint256 coverageScore;
        uint256 minAggregate;
    }

    struct Version { address impl; uint64 activatesAt; bool retired; }

    /// A proposal to change what a symbol means. Takes effect only when a
    /// dependent transition commits. Pull, not push: nothing on the evaluation
    /// path reads the registry.
    struct NotationChange {
        bytes32 oldSymbol;
        bytes32 newSymbol;       // == oldSymbol is legal (same name, new kind or meaning)
        SymbolKind oldKind;
        SymbolKind newKind;
        bytes32 definitionHash;  // keccak256 of the published definition text
        bytes32 prevChangeId;    // committed head of oldSymbol when proposed; 0 = original meaning
        string reason;
        address proposer;
        uint64 proposedAt;
        uint64 committedAt;
        uint256 committedEpoch;  // dialectEpoch after this change took effect; 0 until committed
        NotationStatus status;
    }

    // ------------------------------------------------------------------------
    // Storage
    // ------------------------------------------------------------------------

    // roles that remain: operational capabilities, never governance authority
    bytes32 public constant GUARDIAN = keccak256("GUARDIAN");
    bytes32 public constant OBSERVER = keccak256("OBSERVER");
    bytes32 public constant EXECUTOR = keccak256("EXECUTOR");
    bytes32 public constant SETTLER = keccak256("SETTLER");
    bytes32 public constant OUTCOME_RECORDER = keccak256("OUTCOME_RECORDER");

    mapping(bytes32 => mapping(address => bool)) public hasRole;
    uint256 public pausedUntil;
    uint256 public nextPauseAllowed;
    SystemState public systemState;
    uint256 public evaluatorGasCap = 500_000;
    uint256 public registryDelay;
    mapping(bytes4 => address) public moduleOf;   // selector -> module (delegatecall target)

    // genesis: a bootstrap authority that exists only until sealGenesis()
    address public genesisAdmin;
    bool public genesisSealed;

    // governance-by-transition
    mapping(bytes4 => bytes32) public govCategory;      // action selector -> required transition type
    mapping(bytes32 => bytes) internal govAction;       // transitionId -> exact call data
    mapping(bytes32 => bool) public govApplied;
    /// Council approval of an exact proposal: key = keccak256(abi.encode(transitionId, keccak256(action))).
    mapping(bytes32 => mapping(address => bool)) public govApproved;

    // canonical lineage
    State public declaredState;
    mapping(bytes32 => State) public realizedStates;
    mapping(bytes32 => uint256) public committedIndex;

    mapping(bytes32 => ObservationSnapshot) public observations;
    mapping(bytes32 => ObservationStatus) public observationStatus;
    mapping(bytes32 => Scenario) public scenarios;
    mapping(bytes32 => Goal) public goals;

    mapping(bytes32 => Transition) internal transitions;
    mapping(bytes32 => TransitionLifecycle) public transitionLifecycle;
    mapping(TransitionLifecycle => mapping(TransitionLifecycle => bool)) internal _allowed;

    // policy and mandatory sets + frozen per-transition copies
    mapping(bytes32 => Policy) public policies;
    mapping(bytes32 => bytes32[]) public requiredConstraints;
    mapping(bytes32 => bytes32[]) public requiredObjectives;
    mapping(bytes32 => bytes32[]) public requiredRelevance;
    mapping(bytes32 => bytes32[]) public requiredAttribution;
    mapping(bytes32 => CapabilityRequirement[]) public requiredCapabilities;
    mapping(bytes32 => mapping(bytes32 => uint256)) public groupMultiplier; // type -> group -> WAD (0 = WAD)
    mapping(address => mapping(bytes32 => bool)) public agentCapabilities;

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
    mapping(bytes32 => CommitConditions) public commitConditions;
    mapping(bytes32 => Outcome) public outcomes;

    // agents
    mapping(address => AgentProfile) public agents;
    mapping(address => RepCheckpoint[]) internal repHistory;
    mapping(address => uint256) public acceptedProposals;   // committed transitions this agent proposed

    // layer 1 substrate
    bytes32 public trustMerkleRoot;
    mapping(uint256 => BranchingHealth) public stateHealthHistory;

    // consensus: one signal per agent per round; the latest signal per agent is what counts
    mapping(bytes32 => uint256) public eligibleAt;
    mapping(bytes32 => uint256) public consensusDeadline;
    mapping(bytes32 => mapping(uint256 => mapping(address => ConsensusSignal))) public roundSignals;
    mapping(bytes32 => mapping(uint256 => Tally)) public roundTallies;
    mapping(bytes32 => mapping(address => uint256)) public lastSignalRound1;   // round + 1; 0 = never signalled
    mapping(bytes32 => Tally) public tallies;
    mapping(bytes32 => bytes32) public selectedForParent;
    mapping(bytes32 => bytes32[]) public eligibleSiblings;                    // parent -> competing transitions

    // classifier / niche (informational: never folded into the consensus arithmetic)
    mapping(bytes32 => mapping(address => uint8)) public lastSector1;         // sector + 1; 0 = none
    mapping(address => mapping(Sector => uint256)) public nicheScore;         // accumulated table weight per sector

    // exceptions
    mapping(bytes32 => ExceptionSignal[]) public exceptionSignals;
    mapping(bytes32 => uint256) public unresolvedBlocking;
    mapping(bytes32 => mapping(address => uint256)) public blockingBy;        // transition -> raiser -> unresolved blocking count
    mapping(bytes32 => uint256) public resolutionVotes;
    mapping(bytes32 => mapping(address => bool)) public resolutionVoted;

    // plans
    mapping(bytes32 => Plan) public plans;
    mapping(bytes32 => bytes32[]) public transitionPlans;
    mapping(bytes32 => uint256) public planningDeadline;
    mapping(bytes32 => uint256) public minPlanExpiry;
    mapping(bytes32 => uint256) public replans;

    // settlement
    mapping(bytes32 => mapping(bytes32 => uint256)) public settlementVotes;
    mapping(bytes32 => mapping(address => bool)) public settlementAttested;
    mapping(bytes32 => bytes32[]) public modelProposals;

    // dialect
    mapping(bytes32 => NotationChange) public notationChanges;
    mapping(bytes32 => bytes32) public dialectHead;
    uint256 public dialectEpoch;
    uint256 internal notationNonce;
    mapping(bytes32 => bytes32[]) public dialectDeps;

    // council
    uint256 public epochZero;
    uint256 public epochLength;
    uint256 public councilSeats;
    uint256 public councilSince;                       // epoch the sitting council was installed
    address[] internal _councilMembers;
    mapping(address => bool) public isSeated;
    // weight bound: no single agent may hold more than maxInfluenceShare of the total base influence
    uint256 public totalBaseInfluence;
    uint256 public largestBaseInfluence;
    uint256 public maxInfluenceShare;                  // WAD
    address public positionSource;                     // ILockedHolding
    mapping(address => uint256) public positionStreak;       // consecutive epochs the locked holding was verified
    mapping(address => uint256) public lastPositionEpoch1;   // epoch + 1 of the last verification; 0 = never
    mapping(address => uint256) public lastDefectionEpoch1;  // epoch + 1 of the last confirmed defection; 0 = never

    // ------------------------------------------------------------------------
    // Events
    // ------------------------------------------------------------------------
    event RoleGranted(bytes32 indexed role, address indexed who);
    event RoleRevoked(bytes32 indexed role, address indexed who);
    event PausedSet(uint256 until);
    event PolicySet(bytes32 indexed transitionType);
    event ImplRegistered(RegistryKind indexed kind, bytes32 indexed id, uint256 version, address impl, uint256 activatesAt);
    event EngineRegistered(uint256 version, address impl, uint256 activatesAt);
    event LifecycleChanged(bytes32 indexed id, TransitionLifecycle from, TransitionLifecycle to);
    event ObservationIngested(bytes32 indexed id);
    event ScenarioRegistered(bytes32 indexed id);
    event GoalRegistered(bytes32 indexed id);
    event TransitionRegistered(bytes32 indexed id, bytes32 indexed parentStateId);
    event GovernanceActionProposed(bytes32 indexed id, bytes32 indexed category, bytes action);
    event GovernanceApproved(bytes32 indexed transitionId, bytes32 indexed actionHash, address indexed seat);
    event GovernanceApplied(bytes32 indexed id, bytes4 indexed selector);
    event TransitionEvaluated(bytes32 indexed id, bool constraintsSatisfied, bool objectivesSatisfied);
    event TransitionSelected(bytes32 indexed id);
    event PlanningOpened(bytes32 indexed id, uint256 deadline);
    event PlanSubmitted(bytes32 indexed transitionId, bytes32 indexed planId, address indexed agent);
    event PlansReconciled(bytes32 indexed transitionId, bool compatible);
    event TransitionValidated(bytes32 indexed id);
    event CommitDeclared(bytes32 indexed id);
    event TransitionCommitted(bytes32 indexed id, bytes32 newStateId);
    event ExecutionStarted(bytes32 indexed id);
    event SettlementAttested(bytes32 indexed id, address indexed settler, bytes32 proposalKey);
    event ExecutionSettled(bytes32 indexed id);
    event OutcomeRecorded(bytes32 indexed id, bool successful);
    event AgentRegistered(address indexed agent);
    event ReputationChanged(address indexed agent, int256 delta, int256 newRep);
    event SignalRevised(bytes32 indexed transitionId, uint256 indexed round, address indexed agent, uint256 support);
    event SectorClassified(bytes32 indexed transitionId, address indexed agent, uint256 indexed round, Stance stance, Contribution contribution, Sector sector);
    event SectorReclassified(bytes32 indexed transitionId, address indexed agent, uint256 indexed round, Sector fromSector, Sector toSector);
    event ExceptionSignalSubmitted(bytes32 indexed transitionId, address indexed agent, uint256 index);
    event ExceptionResolved(bytes32 indexed transitionId, uint256 index, ResolutionKind kind, bytes32 evidenceHash);
    event EvaluatorFailed(bytes32 indexed transitionId, bytes32 indexed ruleId);
    event ModelProposalRecorded(bytes32 indexed outcomeId, bytes32 proposalId);
    event SystemStateChanged(SystemState indexed from, SystemState indexed to);
    event TrustMerkleRootUpdated(bytes32 indexed newRoot);
    event BranchingHealthUpdated(uint256 indexed stateIndex, uint256 minAggregate);
    event BranchingCollapseDetected(uint256 indexed stateIndex);
    event NotationProposed(bytes32 indexed id, bytes32 indexed oldSymbol, bytes32 indexed newSymbol, NotationChange change);
    event NotationCommitted(bytes32 indexed id, bytes32 indexed transitionId, uint256 epoch);
    event DialectDependencyDeclared(bytes32 indexed transitionId, bytes32 indexed changeId);
    event CouncilInstalled(uint256 indexed epoch, address[] seats);
    event PositionRecorded(address indexed agent, uint256 indexed epoch, uint256 streak);
    event GenesisSealedEvent();

    // ------------------------------------------------------------------------
    // Access
    // ------------------------------------------------------------------------

    modifier onlyRole(bytes32 role) {
        if (!hasRole[role][msg.sender]) revert Unauthorized(role);
        _;
    }

    /// The only authority over rules. After sealGenesis() it is satisfied solely
    /// by the kernel calling itself from commitGovernance(), i.e. only after a
    /// governance transition has met every term of the commit predicate.
    modifier onlyGovernance() {
        if (!(msg.sender == address(this) || (!genesisSealed && msg.sender == genesisAdmin))) {
            revert Unauthorized(bytes32("GOVERNANCE"));
        }
        _;
    }

    modifier whenNotPaused() {
        if (block.timestamp < pausedUntil) revert IsPaused();
        _;
    }

    /// Ordinary lifecycle calls stop while the substrate is frozen. A reboot
    /// transition is the one thing that must still be able to run.
    modifier whenOperational(bytes32 id) {
        if (block.timestamp < pausedUntil) revert IsPaused();
        if (systemState != SystemState.OPERATIONAL && transitions[id].manifest.transitionType != T_GOV_REBOOT) {
            revert SystemFrozen();
        }
        _;
    }

    // ------------------------------------------------------------------------
    // Lifecycle helpers
    // ------------------------------------------------------------------------

    function _set(bytes32 id, TransitionLifecycle next) internal {
        TransitionLifecycle cur = transitionLifecycle[id];
        if (!_allowed[cur][next]) revert IllegalMove(cur, next);
        transitionLifecycle[id] = next;
        emit LifecycleChanged(id, cur, next);
    }

    function _require(bytes32 id, TransitionLifecycle want) internal view {
        TransitionLifecycle cur = transitionLifecycle[id];
        if (cur != want) revert BadLifecycle(cur);
    }

    function _notExpired(bytes32 id) internal view {
        if (!(block.timestamp < transitions[id].manifest.expiresAt)) revert TransitionExpired();
    }

    function _isPreCommit(TransitionLifecycle s) internal pure returns (bool) {
        return (uint8(s) >= 1 && uint8(s) <= 8) || s == TransitionLifecycle.BLOCKED;
    }

    function _isGovType(bytes32 t) internal pure returns (bool) {
        return t == T_GOV_ADJUSTABLE || t == T_GOV_CONSTITUTION || t == T_GOV_COUNCIL || t == T_GOV_REBOOT;
    }

    function _reg(RegistryKind k) internal view returns (mapping(bytes32 => Version[]) storage r) {
        if (k == RegistryKind.CONSTRAINT) r = _constraintReg;
        else if (k == RegistryKind.OBJECTIVE) r = _objectiveReg;
        else if (k == RegistryKind.RELEVANCE) r = _relevanceReg;
        else r = _attributionReg;
    }

    function _activeVersion(Version[] storage vs) internal view returns (bool found, uint256 v) {
        for (uint256 i = vs.length; i > 0; i--) {
            Version storage x = vs[i - 1];
            if (!x.retired && x.activatesAt <= block.timestamp) return (true, i - 1);
        }
        return (false, NO_VERSION);
    }

    function _observationFresh(bytes32 obsId, uint256 minConf) internal view returns (bool) {
        ObservationSnapshot storage o = observations[obsId];
        return observationStatus[obsId] == ObservationStatus.INGESTED &&
            block.timestamp < o.expiresAt &&
            o.confidence >= minConf;
    }

    // ------------------------------------------------------------------------
    // Reputation, influence, eligibility, weight
    // ------------------------------------------------------------------------

    function _repAt(address agent, uint256 ts) internal view returns (bool found, int256 rep) {
        RepCheckpoint[] storage h = repHistory[agent];
        uint256 lo = 0;
        uint256 hi = h.length;
        while (lo < hi) {
            uint256 mid = (lo + hi) / 2;
            if (h[mid].ts <= ts) lo = mid + 1; else hi = mid;
        }
        if (lo == 0) return (false, 0);
        return (true, h[lo - 1].rep);
    }

    /// I(a) = baseInfluence * (rep - REP_MIN)/(REP_MAX - REP_MIN): monotone, non-negative.
    function influenceAt(address agent, uint256 ts) public view returns (uint256) {
        (bool f, int256 rep) = _repAt(agent, ts);
        if (!f) return 0;
        return agents[agent].baseInfluence * uint256(rep - REP_MIN) / uint256(REP_MAX - REP_MIN);
    }

    /// Group policy = the HIGHEST-MATCHING group multiplier: the single best
    /// multiplier among the agent's groups that are relevant to this transition.
    /// It is not a sum, so joining more groups never raises an agent's weight.
    function _eligible(address agent, bytes32 id) internal view returns (bool ok, uint256 mult) {
        AgentProfile storage ap = agents[agent];
        if (!ap.active) return (false, 0);
        Manifest storage mf = transitions[id].manifest;
        bytes32[] storage rg = mf.relevantGroups;
        for (uint256 i = 0; i < ap.groups.length; i++) {
            for (uint256 j = 0; j < rg.length; j++) {
                if (ap.groups[i] == rg[j]) {
                    uint256 m = groupMultiplier[mf.transitionType][rg[j]];
                    if (m == 0) m = WAD;
                    if (m > mult) mult = m;
                    ok = true;
                }
            }
        }
    }

    function _relevance(bytes32 id, address agent) internal view returns (uint256 total) {
        bytes32[] storage ids = relevanceSet[id];
        if (ids.length == 0) return WAD;
        uint256[] storage vs = context[id].relevanceVersions;
        bytes32 obsId = transitions[id].manifest.observationId;
        for (uint256 i = 0; i < ids.length; i++) {
            if (vs[i] == NO_VERSION) continue;
            address impl = _relevanceReg[ids[i]][vs[i]].impl;
            (bool ok, bytes memory ret) = impl.staticcall{gas: evaluatorGasCap}(
                abi.encodeCall(IRelevanceFunction.evaluate, (agent, id, obsId))
            );
            if (!ok || ret.length != 32) continue;
            uint256 r = abi.decode(ret, (uint256));
            if (r > RELEVANCE_CAP) r = RELEVANCE_CAP;
            total += r;
        }
    }

    /// W_eff = W_raw * R * I(snapshot) * GPolicy, all WAD-scaled.
    function _effectiveWeight(bytes32 id, address agent, uint256 raw, uint256 mult) internal view returns (uint256) {
        uint256 r = _relevance(id, agent);
        uint256 infl = influenceAt(agent, eligibleAt[id]);
        return raw * r / WAD * infl / WAD * mult / WAD;
    }

    /// Every capability the transition type requires must be covered by the plans' agents.
    function capabilityCovered(bytes32 id) public view returns (bool) {
        CapabilityRequirement[] storage reqs = requiredCapabilities[transitions[id].manifest.transitionType];
        bytes32[] storage pIds = transitionPlans[id];
        for (uint256 i = 0; i < reqs.length; i++) {
            uint256 accum = 0;
            for (uint256 j = 0; j < pIds.length; j++) {
                address a = plans[pIds[j]].agent;
                if (agentCapabilities[a][reqs[i].capabilityId]) accum += agents[a].baseInfluence;
            }
            if (accum < reqs[i].minWeightRequired) return false;
        }
        return true;
    }

    function confidenceOf(bytes32 id) public view returns (uint256) {
        Tally storage ty = tallies[id];
        if (ty.totalWeight == 0) return 0;
        return ty.weightedSupport * WAD / ty.totalWeight;
    }

    // ------------------------------------------------------------------------
    // Epochs
    // ------------------------------------------------------------------------

    function currentEpoch() public view returns (uint256) {
        return (block.timestamp - epochZero) / epochLength;
    }

    // ------------------------------------------------------------------------
    // Dialect internals (shared: the predicate reads them, commit runs them)
    // ------------------------------------------------------------------------

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
    ) internal {
        bytes32 id = keccak256(abi.encode(msg.sender, oldSymbol, newSymbol, dialectHead[oldSymbol], ++notationNonce));
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
}
