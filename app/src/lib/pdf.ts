import type { HelperError } from "./api";

/** The certificate of a finished job, rendered as soon as the job ends. */
export type PdfState =
  | { kind: "rendering" }
  | { kind: "saved"; path: string }
  | { kind: "failed"; error: HelperError };
