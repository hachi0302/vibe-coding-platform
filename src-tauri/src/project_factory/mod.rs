mod ai_rules;
mod analysis;
mod docs;
mod env;
mod existing;
mod initialization;
mod install;
mod inventory;
mod materials;
mod path_guard;
mod scaffold;
mod types;

pub use analysis::{analyze_with_agent, analyze_with_agent_progress, build_analysis_prompt};
pub use env::{check_environment, program_path};
pub use existing::{
    existing_project_init_status, finalize_existing_project_initialization,
    prepare_existing_project_initialization,
};
pub use initialization::{
    build_headless_initialization_prompt, initialize_existing_project_with_agent_progress,
};
pub use install::{install_command_for, install_tool};
pub use inventory::{content_sha256, create_filtered_workspace, inspect_project};
pub use materials::read_requirement_materials;
pub use path_guard::{preview_target_path, validate_target_dir};
pub use scaffold::{
    create_project, create_project_with_verification, spring_initializr_dependencies,
};
pub use types::{
    AgentAnalysisProgress, AgentAnalysisResult, AnalyzeProjectRequest, ClarificationAnswer,
    ClarifyingOption, ClarifyingQuestion, CreateProjectRequest, CreateProjectResult, EnvCheckItem,
    ExistingProjectInitPreparation, ExistingProjectInitResult, ExistingProjectInitStatus,
    ExistingProjectInitializationProgress, InventoryFile, ProjectCommand, ProjectInventory,
    ProjectModule, ProjectProfilePayload, ProjectVerificationResult, RecognizedConstraint,
    RequirementMaterialBundle, RequirementMaterialFile, SensitiveFinding,
    StackRecommendationPayload, TechnologyDecision,
};
