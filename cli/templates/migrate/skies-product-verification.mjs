import {
  AvpFail,
  archetype,
  criterion,
  mechanical,
} from "avp-assay";

function productBehaviorProbe(subject) {
  let failure;

  return {
    async act() {
      failure = undefined;
      try {
        await subject.verify();
      } catch (error) {
        failure = error;
      }
    },
    expect: {
      behaviorHolds() {
        if (failure === undefined) return;
        const reason = failure instanceof Error ? failure.message : String(failure);
        throw new AvpFail(`The declared product behavior failed: ${reason}`, { reason });
      },
    },
  };
}

/**
 * Promote one concrete product assertion into an executable Assay verdict.
 * Product-specific semantics remain plain test code; this adapter supplies only the neutral AVP execution shell.
 */
export function productVerification(subjectName, criterionId, statement, verify) {
  if (typeof subjectName !== "string" || !subjectName.trim()) {
    throw new TypeError("productVerification() requires a subject name.");
  }
  if (typeof criterionId !== "string" || !/^[a-z0-9][a-z0-9._-]*$/.test(criterionId)) {
    throw new TypeError("productVerification() requires a stable lowercase criterion id.");
  }
  if (typeof statement !== "string" || !statement.trim()) {
    throw new TypeError("productVerification() requires a semantic criterion statement.");
  }
  if (typeof verify !== "function") {
    throw new TypeError("productVerification() requires an executable verification callback.");
  }

  const productBehavior = archetype(`product-${criterionId}`, "1.0.0", () => {
    criterion(
      criterionId,
      statement,
      { scope: "invariant", substrate: "dom" },
      mechanical(async ({ act, expect }) => {
        await act();
        expect.behaviorHolds();
      }),
    );
  });
  const subject = { name: subjectName, verify };

  return [productBehavior, subject, {
    label: `${subjectName} · ${criterionId}`,
    hooks: (candidate) => ({ probe: () => productBehaviorProbe(candidate) }),
  }];
}
