use crate::model::Grid;

/// Original script column, retained without lossy integer conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize)]
pub enum CobInputColumn {
    Integer(i32),
    FloatBits(u32),
}
impl std::fmt::Display for CobInputColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Integer(value) => write!(f, "{value}"),
            Self::FloatBits(bits) => write!(f, "{}", f32::from_bits(*bits)),
        }
    }
}
impl From<i32> for CobInputColumn {
    fn from(value: i32) -> Self {
        Self::Integer(value)
    }
}
impl From<f32> for CobInputColumn {
    fn from(value: f32) -> Self {
        Self::FloatBits(value.to_bits())
    }
}

/// 纯炮管理器逻辑产生的错误。
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum CobManagerError {
    #[error("炮列表为空")]
    EmptyList,
    #[error("下一门炮的游标无效")]
    InvalidNext,
    #[error("炮落点无效")]
    InvalidTarget,
    #[error("炮输入无效：行 {row}，落点列 {column}")]
    InvalidInput { row: i32, column: CobInputColumn },
    #[error("输入转换失败：{reason}")]
    InputConversion { reason: &'static str },
    #[error("明确发炮输入无效：炮位 ({cr}, {cc}) [{cb}]，落点 ({row}, {column}) [{tb}]", cr=cannon.row, cc=cannon.col, cb=if *cannon_one_based { "1-based" } else { "0-based" }, tb=if *target_one_based { "1-based 行" } else { "0-based 行" })]
    InvalidRawInput {
        cannon: Grid,
        row: i32,
        column: CobInputColumn,
        cannon_one_based: bool,
        target_one_based: bool,
    },
    #[error("炮序模式 {mode:?} 不支持 {operation}")]
    InvalidModeForOperation {
        mode: super::CobSequentialMode,
        operation: &'static str,
    },
    #[error("炮序号 {requested} 无效：当前列表有 {count} 门炮，序号从 1 开始")]
    InvalidSlot { requested: i32, count: usize },
    #[error("炮位 ({row}, {col}) 上没有玉米加农炮", row=.0.row+1, col=.0.col+1)]
    CobNotFound(Grid),
    #[error("炮位 ({row}, {col}) 不在炮列表中", row=.0.row+1, col=.0.col+1)]
    CobNotInList(Grid),
    #[error("没有可用的玉米加农炮")]
    NoReadyCob,
    #[error("没有可恢复的玉米加农炮")]
    NoRecoverableCob,
    #[error("炮位 {grid:?} 已被其他延迟命令预约")]
    Reserved { grid: Grid },
    #[error("炮位 {grid:?} 尚未恢复：还需 {remaining}cs，可等待 {available}cs")]
    Cooling { grid: Grid, remaining: i32, available: i32 },
    #[error("炮位 {0:?} 当前不可恢复")]
    Unrecoverable(Grid),
    #[error("{conclusion}：{evidence}", conclusion = if *recover { "没有可恢复的炮" } else { "没有满足当前发炮条件的炮" })]
    SelectionUnavailable {
        recover: bool,
        evidence: crate::cob::CobSelectionEvidence,
    },
    #[error("没有最近一次发炮记录")]
    NoLatestCob,
    #[error("该操作只适用于屋顶场景")]
    NonRoofScene,
}

/// 炮管理器单项详细操作产生的 backend/core 分层错误。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CobManagerCallError {
    #[error(transparent)]
    Backend(crate::runtime::RuntimeError),
    #[error(transparent)]
    Manager(#[from] CobManagerError),
    #[error("炮位 {cannon:?} 的发炮动作被拒绝：{source}")]
    Rejected {
        cannon: Grid,
        #[source]
        source: crate::runtime::RuntimeError,
    },
    #[error("炮位 {cannon:?} 的延迟发炮注册失败：{source}")]
    Registration {
        cannon: Grid,
        #[source]
        source: crate::runtime::RuntimeError,
    },
}

impl From<CobManagerCallError> for crate::runtime::RuntimeError {
    fn from(error: CobManagerCallError) -> Self {
        Self::from_source(error)
    }
}

impl CobManagerCallError {
    pub(crate) fn stops_batch(&self) -> bool {
        matches!(
            self,
            Self::Backend(_) | Self::Registration { .. } | Self::Manager(CobManagerError::InvalidNext)
        )
    }
    pub(crate) fn cannon(&self) -> Option<Grid> {
        match self {
            Self::Rejected { cannon, .. } | Self::Registration { cannon, .. } => Some(*cannon),
            Self::Manager(
                CobManagerError::CobNotFound(grid)
                | CobManagerError::CobNotInList(grid)
                | CobManagerError::Unrecoverable(grid),
            ) => Some(*grid),
            Self::Manager(CobManagerError::Cooling { grid, .. } | CobManagerError::Reserved { grid }) => Some(*grid),
            _ => None,
        }
    }
}
