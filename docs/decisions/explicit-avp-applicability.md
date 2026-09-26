# Explicit AVP applicability in evidence specs

AVP criteria used to be optional. An agent could omit every tag and obtain a passing proof without considering
the catalog. We now require a decision for every authored failure mode before running or recording its proof.

Use `[avp: criterion-id]` for applicable verifiers. When direct assertions suffice, use `[avp: none]` and an entry
under `## AVP exemptions`: `- FM-n <specific rationale> | reviewed-by: <actual reviewer>`. The human reviews
applicability against the catalog and the assertions before their identity is recorded. The engine validates the
structure and preserves the rationale and reviewer in the receipt; it cannot authenticate the reviewer or judge
the quality of prose. Agents may not invent that review or blanket-exempt old specs.

This is a deliberate exception to optional metadata inside a spec already submitted to the proof engine, not a
new doctor rule requiring production code to carry tags. Existing receipts remain historical artifacts. Existing
and generated specs require an applicability review before new proof runs; generators never pre-approve it.

Synthetic engine/smoke fixtures identify their review as fixture data. Generated authentication cases continue
running directly in the smoke suite, while the proof engine must refuse acceptance until AVP decisions exist.
