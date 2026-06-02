# GitHub Copilot - GLPI Agent Rust

**Agent-specific instructions for GitHub Copilot.**

For ALL universal guidelines (project overview, architecture, workflow, commands, testing, etc.), see **[AGENTS.md](../AGENTS.md)**.

---

## 💡 Prompt Engineering

### Always Include This Context

```
Project: GLPI Agent Rust (glpi-agent-rust)
- Rust rewrite of Perl GLPI Agent
- Cargo workspace: 17 crates under crates/
- Phase-based migration (see ADR-006)
- ALL code in English
- SPDX header required: // SPDX-License-Identifier: GPL-2.0-only
```

### Prompt Templates

**Bug fix:**
```
1. Task: [specific task]
2. Read: [files]
3. Goal: [objective]
4. Need: [request]
```

**New feature:**
```
1. Implement: [feature]
2. Relates to: ADR-XXX, Phase Y
3. Similar: [file paths]
4. Approach: [request]
```

---

## 🎯 Quick Checklists

**Vendor MIB:**
1. Read ADR-003
2. Follow `xerox.rs` pattern
3. Register in `mod.rs` + MIB registry
4. Test with `WalkSession`

**Inventory Category:**
1. Read ADR-006
2. Follow `cpu.rs` pattern
3. Use `#[cfg(target_os = "...")]`
4. Register in `mod.rs` + `task.rs`

---

## 📚 References

- **[AGENTS.md](../AGENTS.md)** ← Start here for everything else
