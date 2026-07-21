mod ai_rules;
mod analysis;
mod artifact_plan;
mod context_memory;
mod docs;
mod document_templates;
mod env;
mod existing;
mod initialization;
mod initialization_state;
mod install;
mod inventory;
mod materials;
mod path_guard;
mod scaffold;
mod types;

pub use analysis::{analyze_with_agent, analyze_with_agent_progress, build_analysis_prompt};
pub use artifact_plan::{
    artifact_totals, read_artifact_plan, validate_artifact_plan, validate_staged_artifacts,
};
pub use env::{check_environment, program_path};
pub use existing::{
    existing_project_init_status, finalize_existing_project_initialization,
    prepare_existing_project_initialization,
};
pub use initialization::{
    build_headless_initialization_prompt, build_v4_stage_prompt, evaluate_agent_stage,
    initialize_existing_project_with_agent_progress, AgentRunOutcome, InitializationStage,
    RepairDecision, RepairTracker, StageDecision,
};
pub use initialization_state::{
    install_builtin_skill_designer, install_managed_entries, install_planned_artifacts,
    load_initialization_state, load_ownership_manifest, save_initialization_state,
    save_ownership_manifest, share_agent_assets, state_directory, verify_ownership_manifest,
};
pub use install::{install_command_for, install_tool};
pub use inventory::{content_sha256, create_filtered_workspace, inspect_project};
pub use materials::read_requirement_materials;
pub use path_guard::{preview_target_path, validate_target_dir};
pub use scaffold::{
    create_project, create_project_with_verification, spring_initializr_dependencies,
};
pub use types::{
    AgentAnalysisProgress, AgentAnalysisResult, AgentAssetMode, AgentAssetTarget,
    AnalyzeProjectRequest, ArtifactKind, ArtifactPlan, ArtifactPlanItem, ArtifactTotals,
    ClarificationAnswer, ClarifyingOption, ClarifyingQuestion, CoverageExclusion,
    CreateProjectRequest, CreateProjectResult, EnvCheckItem, EvidenceReference,
    ExistingProjectInitPreparation, ExistingProjectInitResult, ExistingProjectInitStatus,
    ExistingProjectInitializationProgress, InitializationCheckpoint, InitializationRunState,
    InitializationState, InventoryFile, InventorySummary, ManagedAgentAsset, ManagedEntryOwnership,
    OwnedArtifact, OwnershipManifest, ProjectCommand, ProjectInventory, ProjectModule,
    ProjectProfilePayload, ProjectVerificationResult, RecognizedConstraint,
    RequirementMaterialBundle, RequirementMaterialFile, SensitiveFinding,
    StackRecommendationPayload, TechnologyDecision, ValidationIssue,
};
