# Consistency Validation Plan

Purpose

This document defines a reproducible process to validate consistency across the project's canonical documents (especially `AI_CONTEXT.md`) and to reconcile any contradictions (for example UI mode statements). The goal is to produce a short, auditable set of edits and an approval record so future AI agents and contributors can proceed safely.

Scope

- Documents in scope: `AI_CONTEXT.md`, `PROJECT_PLAN.md`, `DOCUMENT_CONSISTENCY.md`, `ONBOARDING_FROM_AI_CONTEXT.md`, `DEVELOPMENT.md`, `README.md`.
- Focus areas: terminology, scope boundaries, UI-mode decisions, phase status, and "DO NOT SUGGEST" lists.

Owners & Approvals

- Author / Owner: Project owner (you). Only changes that you approve should be merged to the canonical master documents.
- Reviewer(s): Optional — a single trusted contributor or reviewer can be designated.

Process Steps

1. Audit
   - For each document in-scope, run a text-scan for defined keywords and terms:
     - `volume balancing` vs `loudness normalization`
     - `single expert UI` vs `Beginner/Intermediate/Expert`
     - `Phase 1`, `Phase 2`, `Phase 3`, etc.
   - Record every mismatch in a short spreadsheet or markdown table (column: file, line excerpt, suggested action).

2. Origin Investigation
   - Use Git locally to find when and why contradictory phrasing was introduced. Commands to run locally:

```powershell
# show commits which touched AI_CONTEXT.md
git log --follow --pretty=format:"%h %ad %an %s" --date=iso -- AI_CONTEXT.md

# open the full diff for a specific commit
git show <commit-hash> -- AI_CONTEXT.md
```

3. Draft Fixes
   - Create a branch `doc/consistency-fix` and prepare small, focused patches that:
     - Re-assert the canonical rule (e.g., "single expert UI") in `AI_CONTEXT.md`.
     - Remove or reword conflicting lines (for example, change "Progressive disclosure - Beginner/Intermediate/Expert modes" to "Progressive disclosure within a single expert UI when exposing advanced options; keep core UX single-expert by default").
     - Add a link to `ONBOARDING_FROM_AI_CONTEXT.md` as a "Quick onboarding" summary.
     - Add a short note referencing `CONSISTENCY_VALIDATION.md` to document the change.

4. Review Loop
   - Submit a PR against `dev` with the changes and a short PR description summarizing contradictions and the proposed canonical wording.
   - Owner (you) or an assigned reviewer signs off by approving the PR.

5. Merge & Record
   - After approval, merge to `dev` and add a one-line `DOCUMENT_CONSISTENCY.md` update logging the change (what changed, why, who approved, date).

6. Post-merge Actions
   - Run a short automation (optional) that verifies the top-level "REQUIRED READING" section includes `ONBOARDING_FROM_AI_CONTEXT.md`.
   - Update any downstream docs or onboarding material that depend on the changed terms.

Deliverables

- `CONSISTENCY_VALIDATION.md` (this file, stored in repo root)
- A PR branch `doc/consistency-fix` with focused patches
- An updated `DOCUMENT_CONSISTENCY.md` entry recording the decision

Minimal Timeline (suggested)

- Audit: 1-2 hours (manual scan + recording mismatches)
- Draft and PR: 1 hour
- Review & approval: depends on owner availability (recommended <24 hours)
- Merge + follow-up: 30-60 minutes

Guiding Principles

- Keep edits atomic and reversible.
- Do not introduce new scope or features in the same change as a consistency fix.
- Prefer clarifying language rather than deleting historical context — preserve the reason for previous wording in the PR description or `DOCUMENT_CONSISTENCY.md`.

Notes about limitations

- I cannot inspect Git history or make commits outside the file edits performed here. To trace the exact commit that introduced the inconsistent wording, run the `git log` and `git show` commands locally as shown above.

Contact / Owner

- File owner: Project owner (please add your name and preferred reviewer in the PR template).

---
Generated: Nov 23, 2025
