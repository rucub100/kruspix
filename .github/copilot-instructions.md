Execute your tasks strictly according to the architectural guidelines, strict boundaries, and build commands documented in the root `/AGENTS.md` file.

When modifying code, you MUST autonomously search the `.agents/skills/` directory to see if there is a relevant capability you should load before writing code.

## Caveman Mode (always on)

Respond terse. All technical substance stays. Only fluff dies.
Drop: filler (just/really/basically), pleasantries, hedging. Keep articles and full sentences (lite mode).
Pattern: [thing] [action] [reason]. [next step].
Code/commits/PRs: write normally. Off: "stop caveman" or "normal mode".
Switch level: "caveman lite|full|ultra". See `.agents/skills/caveman/SKILL.md` for details.

## Git — Read-Only

**AI must NEVER run any git command that mutates state.** This includes (but is not limited to):
`commit`, `push`, `pull`, `fetch`, `merge`, `rebase`, `reset`, `revert`, `cherry-pick`, `tag`, `branch` (create/delete), `stash`, `add`, `rm`, `mv`, `restore`, `switch -c`, `checkout -b`, `config` (write).

Read-only commands are allowed: `git status`, `git log`, `git diff`, `git show`, `git blame`, `git branch -l`, `git tag -l`, `git ls-files`, etc.

When a git write operation is needed, **provide the exact command(s) for the user to run** — never execute them.

---

## Git Commit Messages

After completing any code change, always provide a ready-to-copy **semantic commit message** in a plaintext code block. Use the format:

```
<type>(<scope>): <subject>
```

- **type** — one of: `chore`, `docs`, `feat`, `fix`, `refactor`, `style`, `test`
- **scope** — optional; the affected area (e.g., `arch`, `mm`, `drivers`, `kernel`, `sched`, `irq`, `sync`, `devicetree`, `common`)
- **subject** — imperative, lowercase, no period at the end

If a change spans multiple scopes, omit the scope or use the most significant one. For multi-part changes, provide one commit message per logical unit.