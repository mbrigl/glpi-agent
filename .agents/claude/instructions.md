# Claude Code - GLPI Agent Rust

**Agent-specific instructions for Claude Code.**

For ALL universal guidelines (project overview, architecture, workflow, commands, testing, etc.), see **[AGENTS.md](../AGENTS.md)**.

---

## 🔐 Claude Code Critical Instructions

### Non-Negotiable Operating Rules

**Read Before You Act:**
- Never edit a file you haven't read in this session
- Read end-to-end, not just snippets

**Change Minimally:**
- Don't touch what wasn't asked
- Match existing style exactly

**Prove It Works:**
- Tests must pass
- Never claim "verified" without execution

**Stop When Stuck:**
- After 2 failed attempts: change strategy or ask

### Blast Radius Awareness

**Always state action + blast radius before:**
- `git push` → State branch
- `git reset --hard` → State scope
- `git clean -fd` → State what's deleted
- `rm -rf` → State path

**Example:** *"I will run `git push` to master. Blast radius: affects origin/master."*

---

## 🖥️ Environment-Specific

### Devcontainer Context
- You run in a **VS Code devcontainer** with Rust 1.96.0
- Host `~/.agents` → Container `/home/vscode/.agents` (bind mount)
- Docker available via `docker-outside-of-docker`

### Devcontainer Commands
```bash
cd /workspace/.devcontainer

# Start services
docker compose up -d
docker compose up -d agent  # Start Perl reference agent

# Logs
docker compose logs -f agent

# Test
docker compose exec agent glpi-inventory --json
```

---

## 📚 References

- **[AGENTS.md](../AGENTS.md)** ← Start here for everything else
