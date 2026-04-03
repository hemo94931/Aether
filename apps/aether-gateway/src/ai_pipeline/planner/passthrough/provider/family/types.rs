#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalSameFormatProviderFamily {
    Standard,
    Gemini,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LocalSameFormatProviderSpec {
    pub(crate) api_format: &'static str,
    pub(crate) decision_kind: &'static str,
    pub(crate) report_kind: &'static str,
    pub(crate) family: LocalSameFormatProviderFamily,
    pub(crate) require_streaming: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct LocalSameFormatProviderDecisionInput {
    pub(crate) auth_context: crate::gateway::GatewayControlAuthContext,
    pub(crate) requested_model: String,
    pub(crate) auth_snapshot: crate::gateway::gateway_data::StoredGatewayAuthApiKeySnapshot,
}

#[derive(Debug, Clone)]
pub(crate) struct LocalSameFormatProviderCandidateAttempt {
    pub(crate) candidate: crate::gateway::scheduler::GatewayMinimalCandidateSelectionCandidate,
    pub(crate) candidate_index: u32,
    pub(crate) candidate_id: String,
}
