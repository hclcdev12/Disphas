import type { Acknowledgement } from "../bindings/Acknowledgement";
import type { Disposition } from "../bindings/Disposition";
import type { MethodChoice } from "../bindings/MethodChoice";

/**
 * Warnings the operator must acknowledge to execute `choice`: disposition
 * codes, reserves, then NS_ALL. Mirrors `MethodChoice::required_acknowledgements`
 * in disphas-protocol, only to build the checklist: the helper checks again
 * and refuses with ACK_MISSING if anything is missing.
 */
export function requiredAcknowledgements(
  choice: MethodChoice,
  disposition: Disposition,
): Acknowledgement[] {
  const codes: Acknowledgement[] = [];
  if (disposition.kind === "DestroyRecommended") codes.push(...disposition.reasons);
  codes.push(...choice.reserves);
  if (choice.all_namespaces) codes.push("NS_ALL");
  return codes;
}
