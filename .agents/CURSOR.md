# Cursor - GLPI Agent Rust

**Agent-specific instructions for Cursor AI.**

For ALL universal guidelines (project overview, architecture, workflow, commands, testing, etc.), see **[AGENTS.md](../AGENTS.md)**.

---

## ⚡ Setup

### Recommended VS Code Extensions
1. Rust Analyzer (matklad.rust-analyzer)
2. Error Lens (usernamehw.errorlens)
3. GitLens (eamodio.gitlens)

### Workspace Settings

```json
{
  "rust-analyzer.checkOnSave.command": "clippy",
  "rust-analyzer.cargo.features": "all",
  "editor.formatOnSave": true,
  "[rust]": {
    "editor.defaultFormatter": "rust-lang.rust-analyzer"
  }
}
```

---

## 💬 Usage Tips

**Inline Chat:** Small, focused changes
**Chat Panel:** Complex, multi-file tasks

Always specify:
- File path
- Context from AGENTS.md
- What you've already read

---

## 📚 References

- **[AGENTS.md](../AGENTS.md)** ← Start here for everything else
