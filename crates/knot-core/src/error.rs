use thiserror::Error;

/// Numeric error code with ranges per subsystem.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ErrorCode {
    // Geometry errors 1xx
    InvalidKnotVector = 100,
    InsufficientControlPoints = 101,
    NegativeWeight = 102,
    DegenerateCurve = 103,
    DegenerateSurface = 104,

    // Topology errors 2xx
    OpenShell = 200,
    NonManifoldEdge = 201,
    InconsistentOrientation = 202,
    DanglingReference = 203,
    EulerViolation = 204,
    LoopNotClosed = 205,

    // Intersection errors 3xx
    NoConvergence = 300,
    MissedBranch = 301,
    DegenerateIntersection = 302,

    // Operation errors 4xx
    EmptyResult = 400,
    SelfIntersecting = 401,
    UnsupportedConfiguration = 402,
    HistoryConflict = 403,
    OperationTimeout = 404,

    // Input errors 5xx
    MalformedInput = 500,
    UnsupportedFormat = 501,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "E{:03}", *self as u32)
    }
}

/// The kernel's error type. Every fallible operation returns this.
#[derive(Clone, Debug, Error)]
pub enum KernelError {
    #[error("invalid geometry ({code}): {detail}")]
    InvalidGeometry { code: ErrorCode, detail: String },

    #[error("topological inconsistency ({code}): {detail}")]
    TopoInconsistency { code: ErrorCode, detail: String },

    #[error("intersection failure ({code}): {detail}")]
    IntersectionFailure { code: ErrorCode, detail: String },

    #[error("operation failed ({code}): {detail}")]
    OperationFailed { code: ErrorCode, detail: String },

    #[error("invalid input ({code}): {detail}")]
    InvalidInput { code: ErrorCode, detail: String },

    #[error("degenerate configuration ({code}): {detail}")]
    Degenerate { code: ErrorCode, detail: String },

    #[error("numerical failure ({code}): {detail}")]
    NumericalFailure { code: ErrorCode, detail: String },

    #[error("io error: {detail}")]
    Io { detail: String },
}

/// Alias used throughout the kernel.
pub type KResult<T> = Result<T, KernelError>;
