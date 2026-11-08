//! Launch data for the built-in harnesses.

use std::path::Path;

use crate::adapters::harness::rules::COMMON;
use crate::domain::error::Result;
use crate::domain::id::HarnessKind;
use crate::domain::rule::Rule;
use crate::ports::harness::{Harness, LaunchPlan, LaunchRequest};

#[derive(Debug, Clone, Copy)]
pub enum ModelFlag {
    Separate(&'static str),
    Joined(&'static str),
    Unsupported,
}

#[derive(Debug, Clone, Copy)]
pub struct Recipe {
    pub kind: &'static str,
    pub interactive: &'static [&'static str],
    pub full_permission: &'static [&'static str],
    pub model: ModelFlag,
    pub env: &'static [(&'static str, &'static str)],
    pub rules: &'static [Rule],
}

const MODEL: ModelFlag = ModelFlag::Separate("--model");

const RECIPES: &[Recipe] = &[
    Recipe {
        kind: "claude",
        interactive: &[],
        full_permission: &["--permission-mode", "bypassPermissions"],
        model: MODEL,
        env: &[("DISABLE_AUTOUPDATER", "1")],
        rules: COMMON,
    },
    Recipe {
        kind: "codex",
        interactive: &[],
        full_permission: &["--dangerously-bypass-approvals-and-sandbox"],
        model: MODEL,
        env: &[],
        rules: COMMON,
    },
    Recipe {
        kind: "cursor",
        interactive: &["--trust"],
        full_permission: &["--force"],
        model: MODEL,
        env: &[],
        rules: COMMON,
    },
    Recipe {
        kind: "devin",
        interactive: &["--respect-workspace-trust", "false"],
        full_permission: &["--permission-mode", "dangerous"],
        model: ModelFlag::Joined("--model"),
        env: &[],
        rules: COMMON,
    },
    Recipe {
        kind: "opencode",
        interactive: &[],
        full_permission: &["--auto"],
        model: MODEL,
        env: &[("OPENCODE_DISABLE_AUTOUPDATE", "1")],
        rules: COMMON,
    },
    Recipe {
        kind: "omp",
        interactive: &[],
        full_permission: &["--approval-mode=yolo"],
        model: MODEL,
        env: &[],
        rules: COMMON,
    },
    Recipe {
        kind: "command-code",
        interactive: &["--trust", "--no-auto-update", "--skip-onboarding"],
        full_permission: &["--permission-mode", "yolo"],
        model: MODEL,
        env: &[],
        rules: COMMON,
    },
    Recipe {
        kind: "cline",
        interactive: &[],
        full_permission: &["--auto-approve", "true"],
        model: ModelFlag::Unsupported,
        env: &[],
        rules: COMMON,
    },
    Recipe {
        kind: "copilot",
        interactive: &["--no-auto-update", "--no-ask-user"],
        full_permission: &["--yolo"],
        model: MODEL,
        env: &[],
        rules: COMMON,
    },
    Recipe {
        kind: "gemini",
        interactive: &["--skip-trust"],
        full_permission: &["--approval-mode", "yolo"],
        model: MODEL,
        env: &[],
        rules: COMMON,
    },
    Recipe {
        kind: "pi",
        interactive: &[],
        full_permission: &[],
        model: MODEL,
        env: &[],
        rules: COMMON,
    },
];

/// Every built-in recipe.
pub fn recipes() -> &'static [Recipe] {
    RECIPES
}

/// The built-in recipe with this name.
pub fn recipe(kind: &str) -> Option<Recipe> {
    RECIPES.iter().find(|recipe| recipe.kind == kind).copied()
}

impl Harness for Recipe {
    fn kind(&self) -> HarnessKind {
        HarnessKind::from_static(self.kind)
    }

    fn launch(&self, request: &LaunchRequest) -> Result<LaunchPlan> {
        let mut argv = vec![display(&request.executable)];
        argv.extend(self.interactive.iter().map(ToString::to_string));
        if request.full_permission {
            argv.extend(self.full_permission.iter().map(ToString::to_string));
        }
        if let Some(model) = &request.model {
            match self.model {
                ModelFlag::Separate(flag) => argv.extend([flag.to_owned(), model.clone()]),
                ModelFlag::Joined(flag) => argv.push(format!("{flag}={model}")),
                ModelFlag::Unsupported => {}
            }
        }
        argv.extend(request.extra_args.iter().cloned());
        Ok(LaunchPlan {
            argv,
            env: self
                .env
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        })
    }

    fn rules(&self) -> &[Rule] {
        self.rules
    }
}

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn built_in(kind: &str) -> Recipe {
        recipe(kind).unwrap()
    }

    fn request(model: Option<&str>, full_permission: bool) -> LaunchRequest {
        LaunchRequest {
            executable: PathBuf::from("/bin/agent"),
            model: model.map(str::to_owned),
            full_permission,
            extra_args: Vec::new(),
        }
    }

    #[test]
    fn full_permission_is_added_only_on_request() {
        let claude = built_in("claude");
        assert_eq!(
            claude.launch(&request(None, false)).unwrap().argv,
            ["/bin/agent"]
        );
        assert_eq!(
            claude.launch(&request(None, true)).unwrap().argv,
            ["/bin/agent", "--permission-mode", "bypassPermissions"]
        );
    }

    #[test]
    fn devin_takes_its_model_in_the_joined_form() {
        let argv = built_in("devin")
            .launch(&request(Some("m1"), false))
            .unwrap()
            .argv;
        assert_eq!(argv.last().unwrap(), "--model=m1");
    }

    #[test]
    fn self_update_is_disabled_for_claude_and_opencode() {
        for (kind, variable) in [
            ("claude", "DISABLE_AUTOUPDATER"),
            ("opencode", "OPENCODE_DISABLE_AUTOUPDATE"),
        ] {
            let plan = built_in(kind).launch(&request(None, false)).unwrap();
            assert_eq!(plan.env, [(variable.to_owned(), "1".to_owned())]);
        }
    }
}
