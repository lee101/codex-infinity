//! Shared command-line flags used by both interactive and non-interactive Codex entry points.

use crate::SandboxModeCliArg;
use clap::Args;
use codex_protocol::config_types::ProfileV2Name;
use codex_protocol::config_types::ShellEnvironmentPolicy;
use codex_protocol::config_types::ShellEnvironmentPolicyInherit;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum YoloMode {
    #[default]
    None,
    Yolo,
    Yolo2,
    Yolo3,
    Yolo4,
}

impl YoloMode {
    pub fn bypasses_approvals_and_sandbox(self) -> bool {
        self >= Self::Yolo
    }

    pub fn disables_command_timeouts(self) -> bool {
        self >= Self::Yolo2
    }

    pub fn uses_full_host_environment(self) -> bool {
        self >= Self::Yolo3
    }

    pub fn streams_command_output_directly(self) -> bool {
        self >= Self::Yolo4
    }

    pub fn shell_environment_policy_override(self) -> Option<ShellEnvironmentPolicy> {
        self.uses_full_host_environment()
            .then(|| ShellEnvironmentPolicy {
                inherit: ShellEnvironmentPolicyInherit::All,
                ignore_default_excludes: true,
                exclude: Vec::new(),
                r#set: std::collections::HashMap::new(),
                include_only: Vec::new(),
                use_profile: false,
            })
    }
}

#[derive(Args, Clone, Debug, Default)]
pub struct SharedCliOptions {
    /// Optional image(s) to attach to the initial prompt.
    #[arg(
        long = "image",
        short = 'i',
        value_name = "FILE",
        value_delimiter = ',',
        num_args = 1..
    )]
    pub images: Vec<PathBuf>,

    /// Model the agent should use.
    #[arg(long, short = 'm')]
    pub model: Option<String>,

    /// Use open-source provider.
    #[arg(long = "oss", default_value_t = false)]
    pub oss: bool,

    /// Specify which local provider to use (lmstudio or ollama).
    /// If not specified with --oss, will use config default or show selection.
    #[arg(long = "local-provider")]
    pub oss_provider: Option<String>,

    /// Layer $CODEX_HOME/<name>.config.toml on top of the base user config.
    #[arg(long = "profile", short = 'p')]
    pub config_profile_v2: Option<ProfileV2Name>,

    /// Select the sandbox policy to use when executing model-generated shell
    /// commands.
    #[arg(long = "sandbox", short = 's')]
    pub sandbox_mode: Option<SandboxModeCliArg>,

    /// Skip all confirmation prompts and execute commands without sandboxing.
    /// EXTREMELY DANGEROUS. Intended solely for running in environments that are externally sandboxed.
    #[arg(
        long = "dangerously-bypass-approvals-and-sandbox",
        alias = "yolo",
        default_value_t = false
    )]
    pub dangerously_bypass_approvals_and_sandbox: bool,

    /// Like --yolo, and disable command timeouts.
    #[arg(long = "yolo2", default_value_t = false)]
    pub yolo2: bool,

    /// Like --yolo2, and pass through the full host environment.
    #[arg(long = "yolo3", default_value_t = false)]
    pub yolo3: bool,

    /// Like --yolo3, and stream command stdout/stderr directly.
    #[arg(long = "yolo4", default_value_t = false)]
    pub yolo4: bool,

    /// Run enabled hooks without requiring persisted hook trust for this invocation.
    /// DANGEROUS. Intended only for automation that already vets hook sources.
    #[arg(long = "dangerously-bypass-hook-trust", default_value_t = false)]
    pub bypass_hook_trust: bool,

    /// Tell the agent to use the specified directory as its working root.
    #[clap(long = "cd", short = 'C', value_name = "DIR")]
    pub cwd: Option<PathBuf>,

    /// Additional directories that should be writable alongside the primary workspace.
    #[arg(long = "add-dir", value_name = "DIR", value_hint = clap::ValueHint::DirPath)]
    pub add_dir: Vec<PathBuf>,
}

impl SharedCliOptions {
    pub fn yolo_mode(&self) -> YoloMode {
        if self.yolo4 {
            YoloMode::Yolo4
        } else if self.yolo3 {
            YoloMode::Yolo3
        } else if self.yolo2 {
            YoloMode::Yolo2
        } else if self.dangerously_bypass_approvals_and_sandbox {
            YoloMode::Yolo
        } else {
            YoloMode::None
        }
    }

    pub fn inherit_exec_root_options(&mut self, root: &Self) {
        let self_selected_sandbox_mode =
            self.sandbox_mode.is_some() || self.yolo_mode().bypasses_approvals_and_sandbox();
        let Self {
            images,
            model,
            oss,
            oss_provider,
            config_profile_v2,
            sandbox_mode,
            dangerously_bypass_approvals_and_sandbox,
            yolo2,
            yolo3,
            yolo4,
            bypass_hook_trust,
            cwd,
            add_dir,
        } = self;
        let Self {
            images: root_images,
            model: root_model,
            oss: root_oss,
            oss_provider: root_oss_provider,
            config_profile_v2: root_config_profile_v2,
            sandbox_mode: root_sandbox_mode,
            dangerously_bypass_approvals_and_sandbox: root_dangerously_bypass_approvals_and_sandbox,
            yolo2: root_yolo2,
            yolo3: root_yolo3,
            yolo4: root_yolo4,
            bypass_hook_trust: root_bypass_hook_trust,
            cwd: root_cwd,
            add_dir: root_add_dir,
        } = root;

        if model.is_none() {
            model.clone_from(root_model);
        }
        if *root_oss {
            *oss = true;
        }
        if oss_provider.is_none() {
            oss_provider.clone_from(root_oss_provider);
        }
        if config_profile_v2.is_none() {
            config_profile_v2.clone_from(root_config_profile_v2);
        }
        if sandbox_mode.is_none() {
            *sandbox_mode = *root_sandbox_mode;
        }
        if !self_selected_sandbox_mode {
            *dangerously_bypass_approvals_and_sandbox =
                *root_dangerously_bypass_approvals_and_sandbox;
            *yolo2 = *root_yolo2;
            *yolo3 = *root_yolo3;
            *yolo4 = *root_yolo4;
        }
        if !*bypass_hook_trust {
            *bypass_hook_trust = *root_bypass_hook_trust;
        }
        if cwd.is_none() {
            cwd.clone_from(root_cwd);
        }
        if !root_images.is_empty() {
            let mut merged_images = root_images.clone();
            merged_images.append(images);
            *images = merged_images;
        }
        if !root_add_dir.is_empty() {
            let mut merged_add_dir = root_add_dir.clone();
            merged_add_dir.append(add_dir);
            *add_dir = merged_add_dir;
        }
    }

    pub fn apply_subcommand_overrides(&mut self, subcommand: Self) {
        let subcommand_selected_sandbox_mode = subcommand.sandbox_mode.is_some()
            || subcommand.yolo_mode().bypasses_approvals_and_sandbox();
        let Self {
            images,
            model,
            oss,
            oss_provider,
            config_profile_v2,
            sandbox_mode,
            dangerously_bypass_approvals_and_sandbox,
            yolo2,
            yolo3,
            yolo4,
            bypass_hook_trust,
            cwd,
            add_dir,
        } = subcommand;

        if let Some(model) = model {
            self.model = Some(model);
        }
        if oss {
            self.oss = true;
        }
        if let Some(oss_provider) = oss_provider {
            self.oss_provider = Some(oss_provider);
        }
        if let Some(config_profile_v2) = config_profile_v2 {
            self.config_profile_v2 = Some(config_profile_v2);
        }
        if subcommand_selected_sandbox_mode {
            self.sandbox_mode = sandbox_mode;
            self.dangerously_bypass_approvals_and_sandbox =
                dangerously_bypass_approvals_and_sandbox;
            self.yolo2 = yolo2;
            self.yolo3 = yolo3;
            self.yolo4 = yolo4;
        }
        if bypass_hook_trust {
            self.bypass_hook_trust = true;
        }
        if let Some(cwd) = cwd {
            self.cwd = Some(cwd);
        }
        if !images.is_empty() {
            self.images = images;
        }
        if !add_dir.is_empty() {
            self.add_dir.extend(add_dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use pretty_assertions::assert_eq;

    #[derive(Debug, Parser)]
    struct TestCli {
        #[clap(flatten)]
        shared: SharedCliOptions,
    }

    #[test]
    fn parses_numbered_yolo_modes() {
        for (flag, expected) in [
            ("--yolo", YoloMode::Yolo),
            ("--yolo2", YoloMode::Yolo2),
            ("--yolo3", YoloMode::Yolo3),
            ("--yolo4", YoloMode::Yolo4),
        ] {
            let cli = TestCli::parse_from(["test", flag]);
            assert_eq!(expected, cli.shared.yolo_mode());
        }
    }

    #[test]
    fn highest_yolo_mode_wins_when_multiple_are_supplied() {
        let cli = TestCli::parse_from(["test", "--yolo", "--yolo3", "--yolo2"]);

        assert_eq!(YoloMode::Yolo3, cli.shared.yolo_mode());
    }

    #[test]
    fn yolo_capabilities_are_cumulative() {
        assert!(YoloMode::Yolo.bypasses_approvals_and_sandbox());
        assert!(!YoloMode::Yolo.disables_command_timeouts());
        assert!(YoloMode::Yolo2.disables_command_timeouts());
        assert!(YoloMode::Yolo3.uses_full_host_environment());
        assert!(YoloMode::Yolo4.streams_command_output_directly());
    }
}
