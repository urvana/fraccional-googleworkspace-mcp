# Domain Docs

How the engineering skills should consume this repo's domain documentation when exploring the codebase.

## Before exploring, read these

- **`GLOSSARY.md`** at the repo root.
- **`docs/adr/`**: read ADRs that touch the area you're about to work in.

If either doesn't exist, proceed silently. Don't suggest creating it upfront; `/domain-modeling` creates documentation lazily when terms or decisions are resolved.

## File structure

Single-context repo: `GLOSSARY.md` and `docs/adr/` live at the repo root.

## Use the glossary's vocabulary

When your output names a domain concept, use the term defined in `GLOSSARY.md`. Don't drift to synonyms the glossary explicitly avoids. If a concept is missing, reconsider whether it belongs or note the gap for `/domain-modeling`.

## Flag ADR conflicts

If your output contradicts an existing ADR, surface the conflict explicitly rather than silently overriding it.
