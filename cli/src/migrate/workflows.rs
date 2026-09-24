//! Removes the Skies 4 gate from CI workflows. A step that runs `skies check` (through the dotnet tool or not) fails
//! once the tool is gone, and a `dotnet tool restore` fails once the migration deleted the only tool in the manifest.
//! Only those steps go; the rest of each job is the application's and is left as written.

use super::Plan;
use super::scripts;

pub fn migrate(text: &str, relative: &str, plan: &mut Plan) -> Option<String> {
    let restores_removed_manifest = plan.tool_manifest_removed;
    let dead = |command: &str| {
        let command = command.trim();
        gate_command(command) || (restores_removed_manifest && command == "dotnet tool restore")
    };
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();
        if !trimmed.starts_with("- ") {
            out.push_str(line);
            index += 1;
            continue;
        }
        // A sequence item owns the lines indented deeper than its dash (and blank lines between them).
        let indent = line.len() - trimmed.len();
        let mut end = index + 1;
        while end < lines.len() && (lines[end].trim().is_empty() || indent_of(lines[end]) > indent) {
            end += 1;
        }
        while end > index + 1 && lines[end - 1].trim().is_empty() {
            end -= 1;
        }
        let item = &lines[index..end];
        match run_command(item) {
            Some(Run::Inline(command)) if dead(&command) => {
                index = end;
            }
            Some(Run::Block(at)) => {
                let body: Vec<&str> = item[at + 1..].to_vec();
                let kept: Vec<&str> = body.iter().copied().filter(|line| !dead(line)).collect();
                let has_command = kept.iter().any(|line| {
                    let line = line.trim();
                    !line.is_empty() && !line.starts_with('#')
                });
                if kept.len() == body.len() {
                    out.push_str(&item.concat());
                } else if has_command {
                    out.push_str(&item[..=at].concat());
                    out.push_str(&kept.concat());
                }
                index = end;
            }
            _ => {
                out.push_str(line);
                index += 1;
            }
        }
    }
    if out == text {
        return None;
    }
    plan.follow_up_file(
        "removed the Skies gate steps from this workflow; review the jobs that remain (names may still mention it)",
        relative,
    );
    Some(out)
}

enum Run {
    /// `run: <command>` on the step's own line.
    Inline(String),
    /// `run: |` (or `>`): the index of that line within the step; the commands follow it.
    Block(usize),
}

fn run_command(item: &[&str]) -> Option<Run> {
    for (at, line) in item.iter().enumerate() {
        let trimmed = line.trim_start().trim_start_matches("- ").trim_start();
        let Some(value) = trimmed.strip_prefix("run:") else {
            continue;
        };
        let value = value.trim();
        return Some(if value.starts_with('|') || value.starts_with('>') {
            Run::Block(at)
        } else {
            Run::Inline(value.trim_matches(['"', '\'']).to_string())
        });
    }
    None
}

/// Whether a shell line runs the removed gate: every `&&` segment is judged the way package scripts are.
fn gate_command(command: &str) -> bool {
    let mut notes = Vec::new();
    matches!(scripts::rewrite(command, &mut notes), scripts::Outcome::Dropped)
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORKFLOW: &str = "jobs:\n  gate:\n    steps:\n      - uses: actions/checkout@v7\n      - run: npm ci\n      - run: dotnet tool restore\n      - name: affected gate\n        if: github.event_name != 'workflow_dispatch'\n        env:\n          SKY_GATE_BASE: x\n        run: dotnet tool run skies check --task \"CI affected verification\" --affected\n      - name: build\n        run: |\n          dotnet build\n          dotnet tool run skies gate\n";

    #[test]
    fn removes_gate_steps_and_gate_lines() {
        let mut plan = Plan::default();
        let out = migrate(WORKFLOW, "ci.yml", &mut plan).unwrap();
        assert_eq!(
            out,
            "jobs:\n  gate:\n    steps:\n      - uses: actions/checkout@v7\n      - run: npm ci\n      - run: dotnet tool restore\n      - name: build\n        run: |\n          dotnet build\n"
        );
        assert!(migrate(&out, "ci.yml", &mut plan).is_none());
    }

    #[test]
    fn drops_the_tool_restore_when_the_manifest_goes() {
        let mut plan = Plan {
            tool_manifest_removed: true,
            ..Plan::default()
        };
        let out = migrate(WORKFLOW, "ci.yml", &mut plan).unwrap();
        assert!(!out.contains("dotnet tool restore"), "{out}");
    }
}
