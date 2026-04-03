#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalStandardSourceFamily {
    Standard,
    Gemini,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalStandardSourceMode {
    Chat,
    Cli,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LocalStandardSpec {
    pub(crate) api_format: &'static str,
    pub(crate) decision_kind: &'static str,
    pub(crate) report_kind: &'static str,
    pub(crate) family: LocalStandardSourceFamily,
    pub(crate) mode: LocalStandardSourceMode,
    pub(crate) require_streaming: bool,
}

#[derive(Debug, Clone)]
pub(super) struct LocalStandardDecisionInput {
    pub(super) auth_context: crate::gateway::GatewayControlAuthContext,
    pub(super) requested_model: String,
    pub(super) auth_snapshot: crate::gateway::gateway_data::StoredGatewayAuthApiKeySnapshot,
}

#[derive(Debug, Clone)]
pub(super) struct LocalStandardCandidateAttempt {
    pub(super) candidate: crate::gateway::scheduler::GatewayMinimalCandidateSelectionCandidate,
    pub(super) candidate_index: u32,
    pub(super) candidate_id: String,
}
