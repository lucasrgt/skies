import type { Archetype } from "avp-assay";
import type { VerificationOptions } from "avp-assay/react/vitest";

/** A product behavior executed through Assay without hiding its test assertion. */
export interface ProductBehaviorSubject {
  readonly name: string;
  readonly verify: () => void | Promise<void>;
}

/**
 * Promote a concrete product assertion into an executable Assay verification tuple.
 * The callback remains ordinary test code and is the source of the criterion's semantics.
 */
export function productVerification(
  subjectName: string,
  criterionId: string,
  statement: string,
  verify: () => void | Promise<void>,
): readonly [Archetype, ProductBehaviorSubject, VerificationOptions];
