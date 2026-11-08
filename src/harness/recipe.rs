//! Launch data for the built-in harnesses.

use std::path::Path;

use crate::error::Result;
use crate::harness::{Harness, HarnessKind, LaunchPlan, LaunchRequest};

#[derive(Debug, Clone, Copy)]
pub enum ModelFlag {
    Separate(&'static str),
    Joined(&'static str),
    Unsupported,
}

#[derive(Debug, Clone, Copy)]
pub struct Recipe {
    pub kind: HarnessKind,
    pub interactive: &'static [&'static str],
    pub full_permission: &'static [&'static str],
    pub model: ModelFlag,
    pub env: &'static [(&'static str, &'static str)],
}

const MODEL: ModelFlag = ModelFlag::Separate("--model");

const RECIPES: &[Recipe] = &[
    Recipe {
        kind: HarnessKind::Claude,
        interactive: &[],
        full_permission: &["--permission-mode", "bypassPermissions"],
        model: MODEL,
        env: &[("DISABLE_AUTOUPDATER", "1")],
    },
    Recipe {
        kind: HarnessKind::Codex,
        interactive: &[],
        full_permission: &["--dangerously-bypass-approvals-and-sandbox"],
        model: MODEL,
        env: &[],
    },
    Recipe {
        kind: HarnessKind::Cursor,
        interactive: &["--trust"],
        full_permission: &["--force"],
        model: MODEL,
        env: &[],
    },
    Recipe {
        kind: HarnessKind::Devin,
        interactive: &["--respect-workspace-trust", "false"],
        full_permission: &["--permission-mode", "dangerous"],
        model: ModelFlag::Joined("--model"),
        env: &[],
    },
    Recipe {
        kind: HarnessKind::OpenCode,
        interactive: &[],
        full_permission: &["--auto"],
        model: MODEL,
        env: &[("OPENCODE_DISABLE_AUTOUPDATE", "1")],
    },
    Recipe {
        kind: HarnessKind::Omp,
        interactive: &[],
        full_permission: &["--approval-mode=yolo"],
        model: MODEL,
        env: &[],
    },
    Recipe {
        kind: HarnessKind::CommandCode,
        interactive: &["--trust", "--no-auto-update", "--skip-onboarding"],
        full_permission: &["--permission-mode", "yolo"],
        model: MODEL,
        env: &[],
    },
    Recipe {
        kind: HarnessKind::Cline,
        interactive: &[],
        full_permission: &["--auto-approve", "true"],
        model: ModelFlag::Unsupported,
        env: &[],
    },
    Recipe {
        kind: HarnessKind::Copilot,
        interactive: &["--no-auto-update", "--no-ask-user"],
        full_permission: &["--yolo"],
        model: MODEL,
        env: &[],
    },
    Recipe {
        kind: HarnessKind::Gemini,
        interactive: &["--skip-trust"],
        full_permission: &["--approval-mode", "yolo"],
        model: MODEL,
        env: &[],
    },
    Recipe {
        kind: HarnessKind::Pi,
        interactive: &[],
        full_permission: &[],
        model: MODEL,
        env: &[],
    },
];

/// Every built-in recipe.
pub fn recipes() -> &'static [Recipe] {
    RECIPES
}

impl Harness for Recipe {
    fn kind(&self) -> HarnessKind {
        self.kind
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
}

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn recipe(kind: HarnessKind) -> Recipe {
        *recipes().iter().find(|recipe| recipe.kind == kind).unwrap()
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
        let claude = recipe(HarnessKind::Claude);
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
        let argv = recipe(HarnessKind::Devin)
            .launch(&request(Some("m1"), false))
            .unwrap()
            .argv;
        assert_eq!(argv.last().unwrap(), "--model=m1");
    }

    #[test]
    fn self_update_is_disabled_for_claude_and_opencode() {
        for (kind, variable) in [
            (HarnessKind::Claude, "DISABLE_AUTOUPDATER"),
            (HarnessKind::OpenCode, "OPENCODE_DISABLE_AUTOUPDATE"),
        ] {
            let plan = recipe(kind).launch(&request(None, false)).unwrap();
            assert_eq!(plan.env, [(variable.to_owned(), "1".to_owned())]);
        }
    }
}
