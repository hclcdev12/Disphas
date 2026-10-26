import type { HelperError } from "./api";
import type { EraseRequest } from "../components/DrivesScreen";

/** How long an erasure waits before it starts, so it can be cancelled (cadre_projet.md §5.4). */
export const CANCEL_DELAY_MS = 10_000;

/** An erasure waiting for its cancellation delay. */
export type Pending = { serial: string; erase: EraseRequest; deadline: number };

/** A launch the helper refused, shown until the operator dismisses it. */
export type LaunchError = { serial: string; model: string; error: HelperError };

/** The certificate of a finished job, rendered as soon as the job ends. */
export type PdfState =
  | { kind: "rendering" }
  | { kind: "saved"; path: string }
  | { kind: "failed"; error: HelperError };
