# Issue tracker: GitHub

Issues and specs live in GitHub Issues in `urvana/fraccional-googleworkspace-mcp`. Use the `gh` CLI. Always specify `--repo urvana/fraccional-googleworkspace-mcp` for `gh issue` and `gh pr` commands: the default repo in this checkout points to upstream.

## Conventions

- **Create**: `gh issue create --repo urvana/fraccional-googleworkspace-mcp --title "..." --body "..."`. Use a heredoc for multi-line bodies.
- **Read**: `gh issue view <number> --repo urvana/fraccional-googleworkspace-mcp --json number,title,body,labels,comments`.
- **List**: `gh issue list --repo urvana/fraccional-googleworkspace-mcp --state open --json number,title,body,labels`, with appropriate label and state filters. Read individual issues for comments.
- **Sub-issues**: `gh issue create --repo urvana/fraccional-googleworkspace-mcp --parent <parent> ...` or `gh issue edit <parent> --repo urvana/fraccional-googleworkspace-mcp --add-sub-issue <child>` (`gh` 2.94+). If unavailable, put `Part of #<parent>` at the top of the child body.
- **Blocking**: use GitHub's native issue dependencies: `gh issue create --repo urvana/fraccional-googleworkspace-mcp --blocked-by <n>,<n> ...` or `gh issue edit <child> --repo urvana/fraccional-googleworkspace-mcp --add-blocked-by <blocker>` (`gh` 2.94+). If unavailable, put `Blocked by: #<n>, #<n>` at the top of the child body. A ticket is unblocked when every blocker is closed.
- **Count open blockers**: `gh api repos/urvana/fraccional-googleworkspace-mcp/issues/<n> --jq .issue_dependencies_summary.blocked_by`; for the fallback, check the referenced issues' states.
- **Comment**: `gh issue comment <number> --repo urvana/fraccional-googleworkspace-mcp --body "..."`.
- **Apply / remove labels**: `gh issue edit <number> --repo urvana/fraccional-googleworkspace-mcp --add-label "..."` / `--remove-label "..."`.
- **Close**: comment with the explanation, then run `gh issue close <number> --repo urvana/fraccional-googleworkspace-mcp`. On an already-closed issue, `gh issue close --comment` drops the comment and still exits successfully.

## Pull requests as a triage surface

**PRs as a request surface: no.** Set this to `yes` if external PRs should be treated as feature requests.

## Skill operations

- **Publish to the issue tracker**: create a GitHub issue.
- **Fetch the relevant ticket**: read the issue as above.
- **Wayfinding**: use an issue labelled `wayfinder:map` as the map, with sub-issues labelled `wayfinder:<type>` (`research`, `prototype`, `grilling`, or `task`). If sub-issues are unavailable, add children to a task list in the map and put `Part of #<map>` in each child. Claim a ticket by assigning it to `@me`. Resolve it by commenting, closing it, then appending a context pointer to the map's Decisions-so-far. Select open, unassigned children without open blockers in map order.
