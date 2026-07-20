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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExistingProjectInitResult {
    pub project_path: String,
    pub layers: super::docs::ProjectLayers,
    pub detected_stack: Vec<String>,
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
    pub marker_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExistingProjectInitializationProgress {
    pub project_path: String,
    pub phase: String,
    pub percent: u8,
    pub detail: String,
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
