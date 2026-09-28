use soroban_sdk::contracttype;

// ============================================================
// Shared Error Types
// ============================================================

/// Common errors that can occur across all contracts
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SharedError {
    // ===== Authorization Errors =====
    /// Caller is not authorized to perform this action
    Unauthorized = 1000,
    /// The provided signature is invalid
    InvalidSignature = 1001,
    /// The caller does not have the required role
    InsufficientRole = 1002,

    // ===== Validation Errors =====
    /// Invalid input provided
    InvalidInput = 2000,
    /// Invalid amount (must be > 0)
    InvalidAmount = 2001,
    /// Invalid address provided
    InvalidAddress = 2002,
    /// Invalid duration (must be > 0)
    InvalidDuration = 2003,
    /// Invalid state for this operation
    InvalidState = 2004,
    /// Invalid asset provided
    InvalidAsset = 2005,

    // ===== Existence Errors =====
    /// The requested resource was not found
    NotFound = 3000,
    /// The resource already exists
    AlreadyExists = 3001,
    /// The resource is not in the expected state
    UnexpectedState = 3002,

    // ===== Balance Errors =====
    /// Insufficient balance for this operation
    InsufficientBalance = 4000,
    /// Insufficient funds available
    InsufficientFunds = 4001,
    /// Balance limit would be exceeded
    BalanceLimitExceeded = 4002,

    // ===== Operation Errors =====
    /// Operation failed
    OperationFailed = 5000,
    /// Operation timed out
    Timeout = 5001,
    /// Operation is not supported
    UnsupportedOperation = 5002,
    /// Operation is paused
    Paused = 5003,
    /// Operation would cause overflow
    ArithmeticOverflow = 5004,

    // ===== Permission Errors =====
    /// Access denied
    AccessDenied = 6000,
    /// Resource is locked
    Locked = 6001,
    /// Resource is frozen
    Frozen = 6002,

    // ===== System Errors =====
    /// Internal system error
    InternalError = 9000,
    /// Contract is not initialized
    NotInitialized = 9001,
    /// Contract is already initialized
    AlreadyInitialized = 9002,
}

impl SharedError {
    /// Check if the error is a validation error
    pub fn is_validation_error(&self) -> bool {
        matches!(self, SharedError::InvalidInput | SharedError::InvalidAmount | SharedError::InvalidAddress | SharedError::InvalidDuration | SharedError::InvalidState | SharedError::InvalidAsset)
    }

    /// Check if the error is an existence error
    pub fn is_existence_error(&self) -> bool {
        matches!(self, SharedError::NotFound | SharedError::AlreadyExists | SharedError::UnexpectedState)
    }

    /// Check if the error is a balance error
    pub fn is_balance_error(&self) -> bool {
        matches!(self, SharedError::InsufficientBalance | SharedError::InsufficientFunds | SharedError::BalanceLimitExceeded)
    }

    /// Check if the error is a system error
    pub fn is_system_error(&self) -> bool {
        matches!(self, SharedError::InternalError | SharedError::NotInitialized | SharedError::AlreadyInitialized)
    }

    /// Get the error code as a string
    pub fn code(&self) -> &'static str {
        match self {
            SharedError::Unauthorized => "UNAUTHORIZED",
            SharedError::InvalidSignature => "INVALID_SIGNATURE",
            SharedError::InsufficientRole => "INSUFFICIENT_ROLE",
            SharedError::InvalidInput => "INVALID_INPUT",
            SharedError::InvalidAmount => "INVALID_AMOUNT",
            SharedError::InvalidAddress => "INVALID_ADDRESS",
            SharedError::InvalidDuration => "INVALID_DURATION",
            SharedError::InvalidState => "INVALID_STATE",
            SharedError::InvalidAsset => "INVALID_ASSET",
            SharedError::NotFound => "NOT_FOUND",
            SharedError::AlreadyExists => "ALREADY_EXISTS",
            SharedError::UnexpectedState => "UNEXPECTED_STATE",
            SharedError::InsufficientBalance => "INSUFFICIENT_BALANCE",
            SharedError::InsufficientFunds => "INSUFFICIENT_FUNDS",
            SharedError::BalanceLimitExceeded => "BALANCE_LIMIT_EXCEEDED",
            SharedError::OperationFailed => "OPERATION_FAILED",
            SharedError::Timeout => "TIMEOUT",
            SharedError::UnsupportedOperation => "UNSUPPORTED_OPERATION",
            SharedError::Paused => "PAUSED",
            SharedError::ArithmeticOverflow => "ARITHMETIC_OVERFLOW",
            SharedError::AccessDenied => "ACCESS_DENIED",
            SharedError::Locked => "LOCKED",
            SharedError::Frozen => "FROZEN",
            SharedError::InternalError => "INTERNAL_ERROR",
            SharedError::NotInitialized => "NOT_INITIALIZED",
            SharedError::AlreadyInitialized => "ALREADY_INITIALIZED",
        }
    }
}
