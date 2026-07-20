use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvCheckItem {
    pub tool_id: String,
    pub label: String,
    pub required: bool,
    pub installed: bool,
    pub compatible: bool,
    pub version: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeProjectRequest {
    pub text: String,
    #[serde(default)]
    pub project_name: Option<String>,
    #[serde(default)]
    pub structure_preference: Option<String>,
    #[serde(default)]
    pub clarification_answers: Vec<ClarificationAnswer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClarificationAnswer {
    pub question_id: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognizedConstraint {
    pub id: String,
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClarifyingOption {
    pub value: String,
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    pub recommended: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClarifyingQuestion {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    pub selection_mode: String,
    pub options: Vec<ClarifyingOption>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAnalysisProgress {
    pub phase: String,
    pub percent: u8,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStackRecommendation {
    pub id: String,
    pub title: String,
    pub frontend: Vec<String>,
    pub backend: Vec<String>,
    pub database: Vec<String>,
    pub cache: Vec<String>,
    pub messaging: Vec<String>,
    pub decisions: Vec<TechnologyDecision>,
    pub structure: String,
    pub package_manager: String,
    pub reasons: Vec<String>,
    pub tradeoffs: Vec<String>,
    pub preference_matched: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TechnologyDecision {
    pub category: String,
    pub title: String,
    pub status: String,
    pub choices: Vec<String>,
    pub reason: String,
    pub provision: String,
    #[serde(default)]
    pub trigger: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAnalysisResult {
    #[serde(default)]
    pub provider: String,
    pub recommended: AgentStackRecommendation,
    pub alternatives: Vec<AgentStackRecommendation>,
    pub not_recommended: Vec<AgentStackRecommendation>,
    pub assumptions: Vec<String>,
    pub project_name: String,
    pub project_name_reason: String,
    #[serde(default)]
    pub recognized_constraints: Vec<RecognizedConstraint>,
    #[serde(default)]
    pub clarifying_questions: Vec<ClarifyingQuestion>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackRecommendationPayload {
    pub id: String,
    pub title: String,
    pub frontend: Vec<String>,
    pub backend: Vec<String>,
    pub database: Vec<String>,
    #[serde(default)]
    pub cache: Vec<String>,
    #[serde(default)]
    pub messaging: Vec<String>,
    #[serde(default)]
    pub decisions: Vec<TechnologyDecision>,
    pub structure: String,
    #[serde(default)]
    pub package_manager: Option<String>,
    #[serde(default)]
    pub reasons: Vec<String>,
    #[serde(default)]
    pub tradeoffs: Vec<String>,
    #[serde(default)]
    pub preference_matched: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProfilePayload {
    pub summary: String,
    pub system_type: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectRequest {
    pub project_name: String,
    pub parent_path: String,
    pub frontend_project_name: Option<String>,
    pub backend_project_name: Option<String>,
    #[serde(default)]
    pub concise_requirement: String,
    #[serde(default)]
    pub recognized_constraints: Vec<RecognizedConstraint>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    pub recommendation: StackRecommendationPayload,
    pub profile: ProjectProfilePayload,
    pub agent_choice: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectVerificationResult {
    pub status: String,
    pub checks: Vec<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectResult {
    pub project_paths: Vec<String>,
    pub agent_mode: String,
    pub message: String,
    pub verification: ProjectVerificationResult,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInventory {
    pub schema_version: u32,
    pub project_name: String,
    pub layers: super::docs::ProjectLayers,
    pub modules: Vec<ProjectModule>,
    pub source_roots: Vec<String>,
    pub files: Vec<InventoryFile>,
    pub commands: Vec<ProjectCommand>,
    pub risk_keys: Vec<SensitiveFinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectModule {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub manifests: Vec<String>,
    pub source_roots: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryFile {
    pub path: String,
    pub kind: String,
    pub size: u64,
    pub sha256: String,
    pub module: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCommand {
    pub name: String,
    pub command: String,
    pub cwd: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensitiveFinding {
    pub path: String,
    pub key: String,
    pub kind: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactKind {
    Document,
    Rule,
    Skill,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceReference {
    pub path: String,
    #[serde(default)]
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactPlanItem {
    pub id: String,
    pub kind: ArtifactKind,
    pub layer: String,
    pub topic: String,
    pub target_path: String,
    pub rationale: String,
    pub evidence: Vec<EvidenceReference>,
    pub covers: Vec<String>,
    pub required_sections: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageExclusion {
    pub target: String,
    pub reason: String,
    #[serde(default)]
    pub evidence: Vec<EvidenceReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactPlan {
    pub schema_version: u32,
    pub project_name: String,
    pub artifacts: Vec<ArtifactPlanItem>,
    #[serde(default)]
    pub exclusions: Vec<CoverageExclusion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    pub code: String,
    pub detail: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub stage: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactTotals {
    pub documents: usize,
    pub rules: usize,
    pub skills: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InitializationRunState {
    #[default]
    Preflight,
    SnapshotReady,
    PlanReady,
    DocumentsReady,
    RulesReady,
    SkillsReady,
    Installing,
    Verifying,
    Completed,
    Failed,
    Interrupted,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializationCheckpoint {
    pub state: InitializationRunState,
    #[serde(default)]
    pub artifact_totals: ArtifactTotals,
    pub completed_at_unix_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializationState {
    pub schema_version: u32,
    pub run_id: String,
    pub state: InitializationRunState,
    #[serde(default)]
    pub workspace_path: String,
    #[serde(default)]
    pub attempt: u32,
    #[serde(default)]
    pub process_id: Option<u32>,
    #[serde(default)]
    pub inventory_sha256: Option<String>,
    #[serde(default)]
    pub plan_sha256: Option<String>,
    #[serde(default)]
    pub artifact_totals: ArtifactTotals,
    #[serde(default)]
    pub checkpoints: Vec<InitializationCheckpoint>,
    #[serde(default)]
    pub issues: Vec<ValidationIssue>,
    #[serde(default)]
    pub conflicts: Vec<ValidationIssue>,
    #[serde(default)]
    pub warnings: Vec<ValidationIssue>,
    #[serde(default)]
    pub started_at_unix_ms: u64,
    #[serde(default)]
    pub updated_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnedArtifact {
    pub path: String,
    pub sha256: String,
    pub kind: ArtifactKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedEntryOwnership {
    pub path: String,
    pub block_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedAgentAsset {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentAssetMode {
    RelativeSymlink,
    ManagedCopy,
    Preserved,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAssetTarget {
    pub path: String,
    pub source_path: String,
    pub mode: AgentAssetMode,
    #[serde(default)]
    pub link_target: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventorySummary {
    pub modules: usize,
    pub source_roots: usize,
    pub files: usize,
    pub frontend: bool,
    pub backend: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnershipManifest {
    pub schema_version: u32,
    pub platform_version: String,
    pub run_id: String,
    pub state: InitializationRunState,
    #[serde(default)]
    pub inventory_sha256: String,
    #[serde(default)]
    pub inventory_summary: Option<InventorySummary>,
    #[serde(default)]
    pub plan_sha256: String,
    #[serde(default)]
    pub artifact_totals: ArtifactTotals,
    #[serde(default)]
    pub artifacts: Vec<OwnedArtifact>,
    #[serde(default)]
    pub managed_entries: Vec<ManagedEntryOwnership>,
    #[serde(default)]
    pub agent_assets: Vec<ManagedAgentAsset>,
    #[serde(default)]
    pub agent_asset_targets: Vec<AgentAssetTarget>,
    #[serde(default)]
    pub agent_asset_mode: Option<AgentAssetMode>,
    #[serde(default)]
    pub checkpoints: Vec<InitializationCheckpoint>,
    #[serde(default)]
    pub conflicts: Vec<ValidationIssue>,
    #[serde(default)]
    pub diagnostics: Vec<ValidationIssue>,
    #[serde(default)]
    pub started_at_unix_ms: u64,
    #[serde(default)]
    pub installed_at_unix_ms: u64,
    #[serde(default)]
    pub completed_at_unix_ms: u64,
}

impl Default for OwnershipManifest {
    fn default() -> Self {
        Self {
            schema_version: 4,
            platform_version: env!("CARGO_PKG_VERSION").to_string(),
            run_id: String::new(),
            state: InitializationRunState::Preflight,
            inventory_sha256: String::new(),
            inventory_summary: None,
            plan_sha256: String::new(),
            artifact_totals: ArtifactTotals::default(),
            artifacts: Vec::new(),
            managed_entries: Vec::new(),
            agent_assets: Vec::new(),
            agent_asset_targets: Vec::new(),
            agent_asset_mode: None,
            checkpoints: Vec::new(),
            conflicts: Vec::new(),
            diagnostics: Vec::new(),
            started_at_unix_ms: 0,
            installed_at_unix_ms: 0,
            completed_at_unix_ms: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExistingProjectInitResult {
    pub project_path: String,
    pub status: String,
    pub phase: String,
    pub run_id: String,
    pub percent: u8,
    pub detail: String,
    pub attempt: u32,
    pub sequence: u64,
    pub recoverable: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<ValidationIssue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<ValidationIssue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    pub artifact_totals: ArtifactTotals,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layers: Option<super::docs::ProjectLayers>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub detected_stack: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub generated: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExistingProjectInitPreparation {
    pub project_path: String,
    pub layers: super::docs::ProjectLayers,
    pub detected_stack: Vec<String>,
    pub existing_docs: Vec<String>,
    pub existing_agent_material: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExistingProjectInitStatus {
    pub initialized: bool,
    pub status: String,
    pub phase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marker_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub percent: u8,
    pub detail: String,
    pub attempt: u32,
    pub sequence: u64,
    pub recoverable: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<ValidationIssue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<ValidationIssue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_totals: Option<ArtifactTotals>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExistingProjectInitializationProgress {
    pub project_path: String,
    pub run_id: Option<String>,
    pub phase: String,
    pub percent: u8,
    pub detail: String,
    pub attempt: u32,
    pub sequence: u64,
    pub recoverable: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<ValidationIssue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<ValidationIssue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_totals: Option<ArtifactTotals>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementMaterialFile {
    pub relative_path: String,
    pub absolute_path: String,
    pub kind: String,
    pub included: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementMaterialBundle {
    pub root_path: String,
    pub source_label: String,
    pub text: String,
    pub files: Vec<RequirementMaterialFile>,
    pub warnings: Vec<String>,
}
