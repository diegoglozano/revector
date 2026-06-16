# Agent skills

[Agent Skills](https://qdrant.tech/documentation/skills/) are decision-tree
guides that tell an AI agent *when* and *why* to reach for a capability — not
*how* (that's the docs). This directory ships a skill for using revector to
manage Qdrant collection **schema and config** migrations.

## Install

Copy the skill into your agent's skills directory:

```sh
# Claude Code / Claude Desktop
cp -r skills/revector ~/.claude/skills/
# Cursor
cp -r skills/revector .cursor/skills/
```

Or point your agent at the file directly if it supports skill URLs.

## Scope

`revector/SKILL.md` covers the schema/config side: payload indexes,
hnsw/quantization/optimizer tuning, named-vector add/drop, drift detection,
brownfield adoption (`stamp`), and CI usage. It deliberately defers:

- **re-embedding / model changes** → the upstream [`qdrant-model-migration`](https://github.com/qdrant/skills/tree/main/skills/qdrant-model-migration) skill;
- **moving points between clusters** → [`qdrant/migration`](https://github.com/qdrant/migration).

## Contributing upstream

This skill follows the [`qdrant/skills`](https://github.com/qdrant/skills)
authoring conventions (leaf skill: `name` + `description` with "Use when"
triggers, scenario sections, a "What NOT to Do" close). To propose it upstream,
validate against their checker (`python3 scripts/validate_skills.py`) and open a
PR there; their topic skills are named `qdrant-<topic>`, so it would likely be
submitted as `qdrant-schema-migration`.
