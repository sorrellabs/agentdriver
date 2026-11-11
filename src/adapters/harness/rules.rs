//! Screen rules for the built-in harnesses, ported from brgr
//! (`brgr-cli/src/pane_adapter/screens.rs` and `native.rs`, v2.15.21).
//!
//! [`COMMON`] lists them in brgr's evaluation order; the first rule that
//! recognises a screen decides. Every rule is opt-in by name
//! ([decision 4](https://github.com/sorrellabs/agentdriver/blob/main/docs/decisions.md#4-prompt-answers-are-opt-in)),
//! and a trust answer is only ever for the session's own spawn directory.

use std::path::Path;

use crate::domain::key::Key;
use crate::domain::rule::{Rule, RuleContext, Verdict};
use crate::domain::screen::Screen;
use crate::domain::text::{
    has_continue_notice, is_skip, is_update_offer, keys_between, menu_options, squash, squash_box,
};

/// Claude Code's folder trust ("Accessing workspace: … Yes, I trust this folder").
pub const WORKSPACE_TRUST: Rule = Rule {
    name: "workspace-trust",
    answer: workspace_trust,
};
/// Codex's folder trust ("Trust this folder? … Trust and continue").
pub const CODEX_TRUST: Rule = Rule {
    name: "codex-trust",
    answer: codex_trust,
};
/// GitHub Copilot CLI's folder trust ("Confirm folder trust").
pub const COPILOT_TRUST: Rule = Rule {
    name: "copilot-trust",
    answer: copilot_trust,
};
/// Cursor Agent's "Workspace Trust Required" box, answered with `a`.
pub const CURSOR_TRUST: Rule = Rule {
    name: "cursor-trust",
    answer: cursor_trust,
};
/// Any other trust prompt. Always refused: it is for some other folder.
pub const TRUST_REFUSAL: Rule = Rule {
    name: "trust-refusal",
    answer: trust_refusal,
};
/// A harness that updated itself and asks for a restart; dismissed with `esc`.
pub const UPDATE_APPLIED: Rule = Rule {
    name: "update-applied",
    answer: update_applied,
};
/// Claude Code's Bypass Permissions warning, accepted.
pub const BYPASS_WARNING: Rule = Rule {
    name: "bypass-warning",
    answer: bypass_warning,
};
/// Codex's review of new hooks, answered by trusting all.
pub const CODEX_HOOKS_TRUST: Rule = Rule {
    name: "codex-hooks-trust",
    answer: codex_hooks_trust,
};
/// Claude Code's newly found MCP server, used (the single-server choice first).
pub const MCP_TRUST: Rule = Rule {
    name: "mcp-trust",
    answer: mcp_trust,
};
/// A harness's offer to update itself, declined by its skip option.
pub const UPDATE_OFFER_SKIP: Rule = Rule {
    name: "update-offer-skip",
    answer: update_offer_skip,
};
/// Claude Code 2.1.318's first-run fullscreen renderer offer, declined.
pub const RENDERER_OFFER_SKIP: Rule = Rule {
    name: "renderer-offer-skip",
    answer: renderer_offer_skip,
};
/// A "press Enter to continue" line, with no menu on screen.
pub const CONTINUE_NOTICE: Rule = Rule {
    name: "continue-notice",
    answer: continue_notice,
};

/// Every built-in rule, in evaluation order.
pub const COMMON: &[Rule] = &[
    WORKSPACE_TRUST,
    CODEX_TRUST,
    COPILOT_TRUST,
    CURSOR_TRUST,
    TRUST_REFUSAL,
    UPDATE_APPLIED,
    BYPASS_WARNING,
    CODEX_HOOKS_TRUST,
    MCP_TRUST,
    UPDATE_OFFER_SKIP,
    RENDERER_OFFER_SKIP,
    CONTINUE_NOTICE,
];

fn workspace_trust(screen: &Screen, context: &RuleContext<'_>) -> Option<Verdict> {
    let text = &screen.text;
    if !trust_workspace_matches(text, context.workspace) {
        return None;
    }
    let options = menu_options(text);
    let yes = options
        .iter()
        .position(|option| option.text.starts_with("Yes, I trust this folder"));
    let selected = options.iter().position(|option| option.selected);
    let keys = if let (Some(selected), Some(yes)) = (selected, yes) {
        // A numbered menu: move to "Yes" from wherever the cursor is.
        with_enter(keys_between(selected, yes))
    } else {
        // The unnumbered form starts on "No, exit", one row above "Yes".
        let flat = squash(text);
        let on_yes = ["❯", "›", ">"]
            .iter()
            .any(|cursor| flat.contains(&format!("{cursor}Yes,Itrustthisfolder")));
        if on_yes {
            vec![Key::Enter]
        } else {
            vec![Key::Down, Key::Enter]
        }
    };
    Some(Verdict::Answer(keys))
}

/// Whether the screen is Claude's trust prompt for exactly this workspace.
fn trust_workspace_matches(screen: &str, workspace: &Path) -> bool {
    // A narrow pane wraps even the option text, so look for it without spaces.
    if !squash(screen).contains("Yes,Itrustthisfolder") {
        return false;
    }
    shown_by_rows(screen, workspace) || shown_in_a_narrow_pane(screen, workspace)
}

/// The path wraps across rows, and a wrap at a space drops that space. Walk the
/// workspace path through the rows, accepting that one space at each row break,
/// so a longer or different path never matches.
fn shown_by_rows(screen: &str, workspace: &Path) -> bool {
    let Some((_, content)) = screen.split_once("Accessing workspace:") else {
        return false;
    };
    let workspace = workspace.to_string_lossy();
    let mut rest = workspace.trim_end_matches('/');
    let mut rows = 0;
    for row in content
        .lines()
        .map(str::trim)
        .skip_while(|row| row.is_empty())
    {
        if row.is_empty() || rest.is_empty() {
            break;
        }
        if rows > 0 {
            rest = rest.strip_prefix(' ').unwrap_or(rest);
        }
        rest = match rest.strip_prefix(row) {
            Some(remaining) => remaining,
            None => match rest.strip_prefix(row.trim_end_matches('/')) {
                Some(remaining) if remaining.is_empty() => remaining,
                _ => return false,
            },
        };
        rows += 1;
    }
    rows > 0 && rest.is_empty()
}

/// In a pane only a few columns wide the header wraps too, so rows no longer
/// mark where the path ends. Compare with all spaces removed, and require the
/// prompt's next sentence right after the path so a longer path cannot match.
fn shown_in_a_narrow_pane(screen: &str, workspace: &Path) -> bool {
    let flat = squash(screen);
    let Some((_, after)) = flat.split_once("Accessingworkspace:") else {
        return false;
    };
    let workspace = squash(&workspace.to_string_lossy());
    after
        .strip_prefix(workspace.trim_end_matches('/'))
        .is_some_and(|rest| rest.trim_start_matches('/').starts_with("Quicksafetycheck"))
}

/// Accepted only for the session's workspace. In a worktree Codex adds that
/// trust applies to the repository root, which must then be exactly the
/// primary checkout the caller named.
fn codex_trust(screen: &Screen, context: &RuleContext<'_>) -> Option<Verdict> {
    let flat = squash(&screen.text);
    if !flat.contains("Trustthisfolder?") || !flat.contains("Trustandcontinue") {
        return None;
    }
    let workspace = squash(&context.workspace.to_string_lossy());
    let primary = context
        .primary_checkout
        .map(|primary| squash(&primary.to_string_lossy()));
    // The path must end where the prompt's own sentence begins, so a longer
    // path ("/repo/task-evil", "/repo/task/sub") is not the session's workspace.
    let shown = flat.split_once("Folderaccess").is_some_and(|(_, after)| {
        after
            .strip_prefix(workspace.trim_end_matches('/'))
            .map(|rest| rest.trim_start_matches('/'))
            .is_some_and(|rest| {
                rest.starts_with("Trustthisfolder")
                    || rest.starts_with("Note:")
                        && rest
                            .split_once("repositoryroot:")
                            .and_then(|(_, root)| root.split_once("Trustthisfolder"))
                            .is_some_and(|(root, _)| {
                                primary.as_deref().is_some_and(|primary| {
                                    root.trim_end_matches('/') == primary.trim_end_matches('/')
                                })
                            })
            })
    });
    Some(trust_choice(&screen.text, shown, |text| {
        text.starts_with("Trust and continue")
    }))
}

/// The path is drawn wrapped inside a box, so it is compared with the borders
/// removed, and must be exactly the workspace. "Yes, and remember this folder"
/// is never chosen.
fn copilot_trust(screen: &Screen, context: &RuleContext<'_>) -> Option<Verdict> {
    let flat = squash_box(&screen.text);
    if !flat.contains("Confirmfoldertrust") || !flat.contains("Doyoutrustthefilesinthisfolder?") {
        return None;
    }
    let workspace = squash(&context.workspace.to_string_lossy());
    let shown = flat
        .split_once("Confirmfoldertrust")
        .and_then(|(_, after)| after.split_once("Copilotcanread"))
        .is_some_and(|(path, _)| path.trim_end_matches('/') == workspace.trim_end_matches('/'));
    Some(trust_choice(&screen.text, shown, |text| text == "Yes"))
}

/// No menu: the box names the key, `[a] Trust this workspace`.
fn cursor_trust(screen: &Screen, context: &RuleContext<'_>) -> Option<Verdict> {
    let flat = squash_box(&screen.text);
    if !flat.contains("WorkspaceTrustRequired") || !flat.contains("[a]Trustthisworkspace") {
        return None;
    }
    // Once answered, the box stays drawn above the agent's own screen with this
    // line in place of the key hint; pressing again would type into its input.
    if flat.contains("Trustingworkspace") {
        return None;
    }
    let workspace = squash(&context.workspace.to_string_lossy());
    let shown = flat
        .split_once("Doyoutrustthecontentsofthisdirectory?")
        .and_then(|(_, after)| after.split_once("[a]Trustthisworkspace"))
        .is_some_and(|(path, _)| {
            // A wide pane draws a selection marker before `[a]`, after the path.
            let path = path.trim_end_matches(['▶', '▸', '❯', '›', '>']);
            path.trim_end_matches('/') == workspace.trim_end_matches('/')
        });
    Some(if shown {
        Verdict::Answer(vec![Key::Char('a')])
    } else {
        Verdict::Refuse
    })
}

fn trust_refusal(screen: &Screen, _context: &RuleContext<'_>) -> Option<Verdict> {
    squash(&screen.text.to_lowercase())
        .contains("trustthisfolder")
        .then_some(Verdict::Refuse)
}

/// The dialog carries an `esc` affordance; Enter ("ok") is what quit OpenCode.
fn update_applied(screen: &Screen, _context: &RuleContext<'_>) -> Option<Verdict> {
    let lower = screen.text.to_lowercase();
    (lower.contains("update complete")
        && lower.contains("restart")
        && screen
            .text
            .lines()
            .any(|line| matches!(line.trim().to_lowercase().as_str(), "ok" | "esc")))
    .then(|| Verdict::Answer(vec![Key::Esc]))
}

fn bypass_warning(screen: &Screen, _context: &RuleContext<'_>) -> Option<Verdict> {
    let lower = screen.text.to_lowercase();
    if !lower.contains("bypass permissions mode") {
        return None;
    }
    move_to(&screen.text, |text| text.starts_with("yes, i accept"))
}

fn codex_hooks_trust(screen: &Screen, _context: &RuleContext<'_>) -> Option<Verdict> {
    let lower = screen.text.to_lowercase();
    // brgr checks these keywords as one group: an earlier keyword on screen
    // settles it even when its option is missing.
    if lower.contains("bypass permissions mode") || !lower.contains("hooks need review") {
        return None;
    }
    move_to(&screen.text, |text| text.starts_with("trust all"))
}

fn mcp_trust(screen: &Screen, _context: &RuleContext<'_>) -> Option<Verdict> {
    let lower = screen.text.to_lowercase();
    if lower.contains("bypass permissions mode")
        || lower.contains("hooks need review")
        || !lower.contains("mcp server")
    {
        return None;
    }
    // The single-server choice first; "use this and all future" only if it is
    // all there is.
    move_to(&screen.text, |text| text.starts_with("use this mcp server"))
        .or_else(|| move_to(&screen.text, |text| text.starts_with("use this")))
}

/// Runs before the continue notice, so a stray "Press Enter" on the same screen
/// never confirms "Update now". A menu without a skip option is left unknown.
fn update_offer_skip(screen: &Screen, _context: &RuleContext<'_>) -> Option<Verdict> {
    if !is_update_offer(&screen.text.to_lowercase()) {
        return None;
    }
    move_to_skip(&screen.text)
}

/// The cursor starts on "Yes, try it".
fn renderer_offer_skip(screen: &Screen, _context: &RuleContext<'_>) -> Option<Verdict> {
    if !screen
        .text
        .to_lowercase()
        .contains("try the new fullscreen renderer?")
    {
        return None;
    }
    move_to_skip(&screen.text)
}

fn continue_notice(screen: &Screen, _context: &RuleContext<'_>) -> Option<Verdict> {
    let menu = menu_options(&screen.text)
        .iter()
        .any(|option| option.selected);
    (!menu && has_continue_notice(&screen.text)).then(|| Verdict::Answer(vec![Key::Enter]))
}

fn with_enter(mut keys: Vec<Key>) -> Vec<Key> {
    keys.push(Key::Enter);
    keys
}

/// Moves from the selected option to the first one `target` accepts (matched
/// on its lowercased text). `None` without a selected option or a target.
fn move_to(screen: &str, target: impl Fn(&str) -> bool) -> Option<Verdict> {
    let options = menu_options(screen);
    let selected = options.iter().position(|option| option.selected)?;
    let target = options
        .iter()
        .position(|option| target(&option.text.to_lowercase()))?;
    Some(Verdict::Answer(with_enter(keys_between(selected, target))))
}

fn move_to_skip(screen: &str) -> Option<Verdict> {
    let options = menu_options(screen);
    let selected = options.iter().position(|option| option.selected)?;
    let skip = options.iter().position(|option| is_skip(&option.text))?;
    Some(Verdict::Answer(with_enter(keys_between(selected, skip))))
}

/// A folder-trust menu: the option `accept` names when the prompt is for the
/// session's own folder (`shown`); otherwise refused.
fn trust_choice(screen: &str, shown: bool, accept: impl Fn(&str) -> bool) -> Verdict {
    let options = menu_options(screen);
    let target = options.iter().position(|option| accept(&option.text));
    let (true, Some(target)) = (shown, target) else {
        return Verdict::Refuse;
    };
    let selected = options
        .iter()
        .position(|option| option.selected)
        .unwrap_or(target);
    Verdict::Answer(with_enter(keys_between(selected, target)))
}

#[cfg(test)]
mod tests {
    //! Ported from brgr's `screens.rs` and `native.rs` tests. Screens marked
    //! synthetic were written by hand; the others are captured panes.

    use std::path::PathBuf;

    use super::*;
    use crate::core::rules::{Classification, RulePolicy, classify};

    macro_rules! screen {
        ($name:literal) => {
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/testdata/screens/",
                $name
            ))
        };
    }

    fn all_enabled() -> RulePolicy {
        RulePolicy::answering(COMMON.iter().map(|rule| rule.name))
    }

    fn classified_with(text: &str, workspace: &str, primary: Option<&str>) -> Classification {
        let workspace = PathBuf::from(workspace);
        let primary = primary.map(PathBuf::from);
        classify(
            &Screen::new(text),
            &RuleContext {
                workspace: &workspace,
                primary_checkout: primary.as_deref(),
            },
            COMMON,
            &all_enabled(),
        )
    }

    fn classified_in(text: &str, workspace: &str) -> Classification {
        classified_with(text, workspace, None)
    }

    fn classified(text: &str) -> Classification {
        classified_in(text, "/repo/task")
    }

    fn resolved(rule: &'static str, keys: &[Key]) -> Classification {
        Classification::Resolve {
            rule,
            keys: keys.to_vec(),
        }
    }

    fn is_unknown(outcome: &Classification) -> bool {
        matches!(outcome, Classification::Unknown { .. })
    }

    /// The screen of the OpenCode pane that stalled brgr task `38d33b4e`,
    /// kept verbatim apart from trailing blank rows.
    const OPENCODE_UPDATED: &str = "\
                                                                                 ┃                               ┃
                                                                                 ┃  MCP Authentication Required  ┃
                                                                                 ┃                               ┃
                                                                                 ┃  Updating to v2.0.15…         ┃
                                                                                 ┃                               ┃

                              Update Complete                                      esc

                              Successfully updated to OpenCode v2.0.15. Please
                              restart the application.


                                                                                 ok

                     ┃
                     ┃  Ask anything… \"Fix broken tests\"
                     ┃
                     ┃  Build auto · Big Pickle OpenCode Zen
                     ╹▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀
                                                                     tab agents  ctrl+p commands
";

    #[test]
    fn an_applied_self_update_is_dismissed_with_esc_not_enter() {
        assert_eq!(
            classified(OPENCODE_UPDATED),
            resolved("update-applied", &[Key::Esc])
        );
    }

    #[test]
    fn an_mcp_auth_toast_alone_does_not_block_the_run() {
        let toast = OPENCODE_UPDATED
            .lines()
            .filter(|line| !line.contains("Update Complete") && !line.contains("restart"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(classified(&toast), Classification::Clear);
    }

    #[test]
    fn the_claude_trust_prompt_for_the_task_workspace_is_accepted() {
        let screen =
            "Accessing workspace:\n\n/repo/task\n\n❯ No, exit\n  Yes, I trust this folder\n";
        assert_eq!(
            classified(screen),
            resolved("workspace-trust", &[Key::Down, Key::Enter])
        );
        let selected =
            "Accessing workspace:\n\n/repo/task\n\n  No, exit\n❯ Yes, I trust this folder\n";
        assert_eq!(
            classified(selected),
            resolved("workspace-trust", &[Key::Enter])
        );
    }

    /// Captured with Herdr 0.9.0: the menu starts on "No, exit".
    #[test]
    fn claude_2_1_319_trust_for_its_own_folder_moves_to_yes() {
        let screen = screen!("claude-2.1.319-folder-trust.txt");
        assert_eq!(
            classified_in(screen, "/work/task"),
            resolved("workspace-trust", &[Key::Down, Key::Enter])
        );
        assert!(is_unknown(&classified_in(screen, "/work/other")));
    }

    #[test]
    fn a_numbered_claude_trust_menu_is_answered_by_moving_to_yes() {
        let screen = "Accessing workspace:\n\n/repo/task\n\nQuick safety check\n\n❯ 1. Yes, I trust this folder\n  2. No, exit\n";
        assert_eq!(
            classified(screen),
            resolved("workspace-trust", &[Key::Enter])
        );
        let other = screen
            .replace("❯ 1. Yes", "  1. Yes")
            .replace("  2. No, exit", "❯ 2. No, exit");
        assert_eq!(
            classified(&other),
            resolved("workspace-trust", &[Key::Up, Key::Enter])
        );
    }

    #[test]
    fn a_trust_prompt_for_another_folder_is_not_accepted() {
        let screen =
            "Accessing workspace:\n\n/elsewhere\n\n❯ No, exit\n  Yes, I trust this folder\n";
        assert!(is_unknown(&classified(screen)));
    }

    #[test]
    fn trust_path_preserves_spaces_and_rejects_prefixes() {
        let screen = |path: &str| {
            format!("Accessing workspace:\n\n{path}\n\n❯ No, exit\nYes, I trust this folder\n")
        };
        assert!(trust_workspace_matches(
            &screen("/repo/task one/"),
            Path::new("/repo/task one")
        ));
        assert!(!trust_workspace_matches(
            &screen("/repo/taskone"),
            Path::new("/repo/task one")
        ));
        assert!(!trust_workspace_matches(
            &screen("/repo/task-other"),
            Path::new("/repo/task")
        ));
        assert!(trust_workspace_matches(
            &screen("/repo/\ntask"),
            Path::new("/repo/task")
        ));
    }

    /// A pane only a few columns wide (brgr task `e2174e4c`): the header, the
    /// path and even the option text wrap word by word.
    #[test]
    fn trust_prompt_in_a_very_narrow_pane_still_matches() {
        let screen = "Accessing\nworkspace:\n/Users/justn/Library/Application\nSupport/brgr/worktrees/fresh\nrepo/e2174e4c\nQuick\nsafety\ncheck:\nIs this a project\n❯ No, exit\n  Yes, I\n  trust\n  this\n  folder\n";
        let workspace = Path::new(
            "/Users/justn/Library/Application Support/brgr/worktrees/fresh repo/e2174e4c",
        );
        assert!(trust_workspace_matches(screen, workspace));
        assert!(!trust_workspace_matches(
            &screen.replace("e2174e4c", "e2174e4c-extra"),
            workspace
        ));
        assert!(!trust_workspace_matches(
            screen,
            Path::new("/Users/justn/Library/Application Support/brgr/worktrees/fresh repo/other")
        ));
    }

    /// A narrow pane (brgr task `915c0be5`): the path wrapped at "Application
    /// Support", the space went with the wrap, and no blank row follows.
    #[test]
    fn trust_path_wrapped_at_a_space_still_matches() {
        let screen = "Accessing workspace:\n /Users/justn/Library/Application\n Support/brgr/worktrees/dogfood-repo/915c0be5\n Quick safety check: Is this a project you created\n\n ❯ No, exit\n   Yes, I trust this folder\n";
        let workspace = Path::new(
            "/Users/justn/Library/Application Support/brgr/worktrees/dogfood-repo/915c0be5/",
        );
        assert!(trust_workspace_matches(screen, workspace));
        assert!(!trust_workspace_matches(
            screen,
            Path::new("/Users/justn/Library/Application Support/brgr/worktrees/dogfood-repo/other")
        ));
        assert!(!trust_workspace_matches(
            &screen.replace("915c0be5", "915c0be5-extra"),
            workspace
        ));
    }

    /// Verbatim from a real Codex 0.160 pane (brgr dogfood D13), path wrapped.
    const CODEX_FOLDER_TRUST: &str = "\
  Folder access
  /private/tmp/scratchpad/cod
  ex owner repo
  Trust this folder? Codex can read, edit, and run files here, subject to your permission settings.
  Folder settings can run code automatically, even without a model request. Continue only if you
  trust these files. Your trust decision will be saved.
› 1. Trust and continue
  2. Back to Agent Command Center
  enter continue · esc back
";
    const CODEX_FOLDER: &str = "/private/tmp/scratchpad/codex owner repo";

    #[test]
    fn the_codex_trust_prompt_for_the_task_workspace_is_accepted() {
        assert_eq!(
            classified_in(CODEX_FOLDER_TRUST, CODEX_FOLDER),
            resolved("codex-trust", &[Key::Enter])
        );
        // The cursor on the other option still ends on "Trust and continue".
        let moved = CODEX_FOLDER_TRUST
            .replace("› 1. Trust and continue", "  1. Trust and continue")
            .replace("  2. Back", "› 2. Back");
        assert_eq!(
            classified_in(&moved, CODEX_FOLDER),
            resolved("codex-trust", &[Key::Up, Key::Enter])
        );
    }

    #[test]
    fn the_codex_trust_prompt_for_another_folder_is_not_accepted() {
        assert!(is_unknown(&classified_in(CODEX_FOLDER_TRUST, "/elsewhere")));
    }

    #[test]
    fn the_codex_trust_prompt_for_a_longer_path_is_not_accepted() {
        for longer in [
            "/private/tmp/scratchpad/codex owner repo-evil",
            "/private/tmp/scratchpad/codex owner repo/sub",
        ] {
            let screen =
                CODEX_FOLDER_TRUST.replace("/private/tmp/scratchpad/cod\n  ex owner repo", longer);
            assert!(
                is_unknown(&classified_in(&screen, CODEX_FOLDER)),
                "{longer}"
            );
        }
    }

    const CODEX_WORKTREE: &str = "/private/tmp/claude-501/-Users-justn-dev-brgr/b54b7525-6101-4186-a2b9-0e32355d043a/scratchpad/cap-codex-wt";
    const CODEX_REPOSITORY: &str = "/private/tmp/claude-501/-Users-justn-dev-brgr/b54b7525-6101-4186-a2b9-0e32355d043a/scratchpad/cap-codex-repo";

    #[test]
    fn codex_trust_in_a_task_worktree_names_and_accepts_its_repository() {
        assert_eq!(
            classified_with(
                screen!("codex-0.160-worktree-trust.txt"),
                CODEX_WORKTREE,
                Some(CODEX_REPOSITORY)
            ),
            resolved("codex-trust", &[Key::Enter])
        );
    }

    /// Codex 0.166.0 in a worktree: the path has a space, the root is on two
    /// lines, and the second choice is `Back to Agent Command Center`.
    #[test]
    fn codex_0166_trust_in_a_task_worktree_names_and_accepts_its_repository() {
        assert_eq!(
            classified_with(
                screen!("codex-0.166-worktree-trust.txt"),
                "/Users/justn/Library/Application Support/brgr/worktrees/ws/6ca4cbb7",
                Some(
                    "/private/tmp/claude-501/-Users-justn-dev-brgr/2c6e674e-b811-4b53-8577-a4d60a8cc2ab/scratchpad/m226/ws"
                )
            ),
            resolved("codex-trust", &[Key::Enter])
        );
    }

    #[test]
    fn codex_trust_for_a_root_that_is_not_the_worktrees_repository_is_refused() {
        for primary in [None, Some("/private/tmp/other-repo"), Some(CODEX_WORKTREE)] {
            assert!(
                is_unknown(&classified_with(
                    screen!("codex-0.160-worktree-trust.txt"),
                    CODEX_WORKTREE,
                    primary
                )),
                "{primary:?}"
            );
        }
    }

    #[test]
    fn codexs_hook_review_is_answered_by_trusting_all() {
        assert_eq!(
            classified_in(screen!("codex-0.166-hooks-review.txt"), "/somewhere"),
            resolved("codex-hooks-trust", &[Key::Down, Key::Enter])
        );
    }

    const CURSOR_WORKSPACE: &str = "/private/tmp/claude-501/-Users-justn-dev-brgr/2c6e674e-b811-4b53-8577-a4d60a8cc2ab/scratchpad/cur3-work";

    #[test]
    fn cursors_trust_prompt_for_the_task_workspace_is_accepted_with_its_key() {
        assert_eq!(
            classified_in(
                screen!("cursor-2026.10.22-trust-wide.txt"),
                CURSOR_WORKSPACE
            ),
            resolved("cursor-trust", &[Key::Char('a')])
        );
    }

    #[test]
    fn cursors_trust_prompt_for_another_folder_is_not_accepted() {
        for other in [
            "/private/tmp/claude-501/-Users-justn-dev-brgr/2c6e674e-b811-4b53-8577-a4d60a8cc2ab/scratchpad/cur3-wor",
            "/private/tmp/claude-501",
            "/somewhere/else",
        ] {
            assert!(
                is_unknown(&classified_in(
                    screen!("cursor-2026.10.22-trust-wide.txt"),
                    other
                )),
                "{other}"
            );
        }
    }

    #[test]
    fn cursors_answered_trust_box_still_on_screen_is_not_answered_again() {
        assert_eq!(
            classified_in(
                screen!("cursor-2026.10.22-trust.txt"),
                "/private/tmp/claude-501/-Users-justn-dev-brgr/2c6e674e-b811-4b53-8577-a4d60a8cc2ab/scratchpad/cursor-work"
            ),
            Classification::Clear
        );
    }

    const COPILOT_WORKSPACE: &str = "/private/tmp/claude-501/-Users-justn-dev-brgr/b54b7525-6101-4186-a2b9-0e32355d043a/scratchpad/cap-copilot-1791247661";

    #[test]
    fn copilots_trust_prompt_for_the_task_workspace_is_accepted_once() {
        assert_eq!(
            classified_in(screen!("copilot-1.0.86-trust.txt"), COPILOT_WORKSPACE),
            resolved("copilot-trust", &[Key::Enter])
        );
    }

    #[test]
    fn copilots_trust_prompt_for_another_folder_is_not_accepted() {
        for other in [
            "/private/tmp/claude-501/-Users-justn-dev-brgr/b54b7525-6101-4186-a2b9-0e32355d043a/scratchpad/cap-copilot-179124766",
            "/private/tmp/claude-501",
            "/somewhere/else",
        ] {
            assert!(
                is_unknown(&classified_in(screen!("copilot-1.0.86-trust.txt"), other)),
                "{other}"
            );
        }
    }

    #[test]
    fn copilots_idle_screen_needs_no_answer() {
        assert_eq!(
            classified_in(screen!("copilot-1.0.86-idle.txt"), COPILOT_WORKSPACE),
            Classification::Clear
        );
    }

    /// Synthetic: modelled on the Codex update offer described in brgr v2.9.3.
    #[test]
    fn an_update_offer_is_skipped_by_moving_to_the_skip_option() {
        let screen = "\
 Update available! 0.1.0 -> 0.2.0

 › 1. Update now
   2. Skip
   3. Skip until next version
";
        assert_eq!(
            classified(screen),
            resolved("update-offer-skip", &[Key::Down, Key::Enter])
        );
    }

    /// Synthetic.
    #[test]
    fn an_update_offer_with_the_cursor_on_skip_only_presses_enter() {
        assert_eq!(
            classified("Update available\n  1. Update now\n❯ 2. Not now\n"),
            resolved("update-offer-skip", &[Key::Enter])
        );
    }

    /// Synthetic: an offer whose skip option is worded "Later".
    #[test]
    fn an_update_offer_is_skipped_by_its_later_option() {
        assert_eq!(
            classified("Update available\n› 1. Update now\n  2. Later\n"),
            resolved("update-offer-skip", &[Key::Down, Key::Enter])
        );
    }

    /// Synthetic: a stray notice line must not turn an update menu into Enter.
    #[test]
    fn an_update_menu_with_a_continue_line_is_skipped_not_confirmed() {
        assert_eq!(
            classified("Update available\n› 1. Update now\n  2. Skip\nPress Enter to continue\n"),
            resolved("update-offer-skip", &[Key::Down, Key::Enter])
        );
    }

    #[test]
    fn an_update_menu_without_a_skip_option_is_not_guessed() {
        assert!(is_unknown(&classified(
            "Update available\n› 1. Update now\n  2. Show release notes\n"
        )));
    }

    /// Synthetic.
    #[test]
    fn an_agent_question_about_updating_something_is_not_an_update_offer() {
        for screen in [
            "Update the schema now?\n› 1. Yes\n  2. Later\n",
            "Upgrade the dependencies too?\n› 1. Yes\n  2. Skip\n",
            "> 1. update the config, then skip later steps\n  2. next\n",
        ] {
            assert!(is_unknown(&classified(screen)), "{screen}");
        }
    }

    /// Captured from Claude Code 2.1.318's first run; the cursor starts on
    /// "Yes, try it".
    #[test]
    fn claudes_renderer_offer_is_declined_by_moving_to_not_now() {
        assert_eq!(
            classified_in(screen!("claude-2.1.318-renderer-offer.txt"), "/somewhere"),
            resolved("renderer-offer-skip", &[Key::Down, Key::Enter])
        );
    }

    #[test]
    fn a_question_about_a_fullscreen_renderer_without_that_offer_is_not_answered() {
        assert!(is_unknown(&classified_in(
            "Should the fullscreen renderer be the default?\n❯ 1. Yes\n  2. Not now\n",
            "/somewhere"
        )));
    }

    /// Synthetic: Claude Code's documented Bypass Permissions warning.
    #[test]
    fn the_bypass_permissions_warning_is_accepted() {
        let screen = "WARNING: Claude Code running in Bypass Permissions mode\n\nIn Bypass Permissions mode, Claude Code will not ask for your approval.\n\n❯ 1. No, exit\n  2. Yes, I accept\n";
        assert_eq!(
            classified(screen),
            resolved("bypass-warning", &[Key::Down, Key::Enter])
        );
    }

    /// Synthetic: Claude Code's documented new-MCP-server prompt.
    #[test]
    fn a_new_mcp_server_prompt_prefers_the_single_server_choice() {
        let screen = "New MCP server found in .mcp.json: docs\n\n❯ 1. Use this and all future MCP servers in this project\n  2. Use this MCP server\n  3. Continue without using this MCP server\n";
        assert_eq!(
            classified(screen),
            resolved("mcp-trust", &[Key::Down, Key::Enter])
        );
    }

    #[test]
    fn agent_prose_about_an_update_is_not_the_applied_update_dialog() {
        assert_eq!(
            classified("The build finished. Update complete, please restart the dev server.\n"),
            Classification::Clear
        );
    }

    #[test]
    fn sentence_followed_by_a_continue_notice_is_dismissed_with_enter() {
        assert_eq!(
            classified("│ • Startup complete. Press Enter to continue… │\n"),
            resolved("continue-notice", &[Key::Enter])
        );
    }

    /// Synthetic.
    #[test]
    fn a_continue_notice_is_dismissed_with_enter() {
        assert_eq!(
            classified("Welcome to the new release\n\n  Press Enter to continue\n"),
            resolved("continue-notice", &[Key::Enter])
        );
    }

    #[test]
    fn prose_quoting_or_discussing_a_continue_notice_is_left_alone() {
        for prose in [
            "The tool prints \"press enter to continue\" when it stops.\n",
            "I saw press enter to continue in the log.\n",
            "Then it says press enter to continue.\n",
            "Press Enter to continue or Esc to cancel\n",
        ] {
            assert_eq!(classified(prose), Classification::Clear, "{prose:?}");
        }
    }

    #[test]
    fn a_continue_notice_beside_a_menu_is_left_to_the_menu_rules() {
        assert!(is_unknown(&classified(
            "Press Enter to continue\n› 1. Update now\n  2. Install\n"
        )));
    }

    /// Synthetic.
    #[test]
    fn a_menu_no_rule_covers_is_unknown_and_never_pressed() {
        assert_eq!(
            classified("Choose a theme\n› 1. Dark\n  2. Light\n"),
            Classification::Unknown {
                excerpt: "Choose a theme | › 1. Dark | 2. Light".to_owned()
            }
        );
    }

    #[test]
    fn an_ordinary_agent_screen_is_clear() {
        assert_eq!(classified("some output\n❯ \n"), Classification::Clear);
    }

    #[test]
    fn a_rule_the_caller_did_not_enable_is_reported_not_pressed() {
        let workspace = PathBuf::from("/repo/task");
        assert!(matches!(
            classify(
                &Screen::new("Welcome\n\n  Press Enter to continue\n"),
                &RuleContext::new(&workspace),
                COMMON,
                &RulePolicy::default()
            ),
            Classification::Prompt {
                rule: "continue-notice",
                ..
            }
        ));
    }
}
