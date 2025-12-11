Thank you for taking the time to contribute. This project is open to small fixes and ambitious changes alike, and your effort is appreciated either way. By opening a pull request, you agree that your contributions will be distributed under the project’s existing LICENSE (Apache-2.0); if you are unsure what that means for your situation, read the LICENSE file before you start.

Almost everything here is wired through overlapping layers of protection:
redundant tests and checks, hermetic helpers, type-checked contracts, and dense
but targeted documentation (comments, AGENTS files, schema narratives). This is
deliberate. The structure is designed so that you can move quickly—add a probe,
change a helper, tighten a schema—while the system itself catches
inconsistencies and forces you to be explicit about what you meant to do. The
intent is rapid development under constraints, not ceremony for its own sake.

Within that structure, you are encouraged to work boldly as long as you keep a
few ideas in mind:
- Treat the contracts, especially capability catalogs and boundary-object schema as real APIs, not suggestions. If you need to change them, do so deliberately, with matching updates to code, docs, and tests.
- Prefer small, explicit steps over clever rewrites. Wire new work through existing helpers, reuse established patterns, and let the type system and validation tooling guide you.
- Do not commit into the dark. Run the available checks locally (`make test`), read the failures, and only push changes once the board is green.
The repository is built to support significant refactors and new ideas, especially from someone arriving with fresh eyes.

As you work, perpetuate and extend the patterns you find here. When you
understand a corner of the system, leave it better documented than you found
it: add a comment that explains a non-obvious decision, tighten an AGENTS
instruction, or clarify a schema description. When you touch behavior, ask
whether there is a test you can add or extend, even if the existing coverage
would probably catch regressions already. If something is subtle, err on the
side of explaining it twice in two different places. That kind of
over-provisioning is a feature of this project, not a bug—the more you lean
into it, the more resilient Fencerunner becomes for the next contributor—human
or otherwise.
