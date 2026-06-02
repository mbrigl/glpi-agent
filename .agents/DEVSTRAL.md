# Devstral (Mistral Vibe) - GLPI Agent Rust

**Agent-specific instructions for Devstral (Mistral Vibe).**

For ALL universal guidelines (project overview, architecture, workflow, commands, testing, etc.), see **[AGENTS.md](../AGENTS.md)**.

---

## 🛠️ Tool Usage Priority

**ALWAYS use dedicated tools over bash:**

| Use This | Not This | Example |
|----------|----------|---------|
| `read_file` | `bash(cat ...)` | `read_file(path="crates/glpi-core/src/lib.rs")` |
| `grep` | `bash(grep ...)` | `grep(pattern="MibSupport", path="crates/")` |
| `search_replace` | `bash(sed ...)` | SEARCH/REPLACE blocks |
| `bash` with timeout | `bash` without | `bash(command="...", timeout=120)` |

---

## 🎯 Operating Discipline

1. **Read first** - Never edit unread files
2. **Chunk reading** - Use `limit`/`offset` for large files
3. **Exact copies** - Copy text precisely from read files
4. **5+ equals** - Use `=====` in SEARCH/REPLACE
5. **Verify** - Always test your changes

---

## 📊 Devcontainer

```python
# Always use absolute paths and timeouts
bash(command="cd /workspace/.devcontainer && docker compose up -d", timeout=120)

# From container network: use http://glpi (not localhost)
```

---

## 📚 References

- **[AGENTS.md](../AGENTS.md)** ← Start here for everything else
