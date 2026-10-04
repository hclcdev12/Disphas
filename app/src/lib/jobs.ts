import type { Event as HelperEvent } from "../bindings/Event";
import type { FinalStatus } from "../bindings/FinalStatus";
import type { JobLogData } from "../bindings/JobLogData";
import type { JobPhase } from "../bindings/JobPhase";
import type { JobProgressData } from "../bindings/JobProgressData";
import type { JobSummary } from "../bindings/JobSummary";

/** What the interface knows about a job, built only from helper events. */
export type JobView = {
  phase: JobPhase;
  progress: JobProgressData | null;
  logs: JobLogData[];
  status: FinalStatus | null;
};

const EMPTY: JobView = { phase: "Preparing", progress: null, logs: [], status: null };

/** Applies one helper event to the known jobs. */
export function applyEvent(jobs: Record<string, JobView>, event: HelperEvent): Record<string, JobView> {
  if (!("data" in event) || !("job_id" in event.data)) return jobs;
  const id = event.data.job_id;
  const job = jobs[id] ?? EMPTY;
  switch (event.event) {
    case "JobPhase":
      return { ...jobs, [id]: { ...job, phase: event.data.phase, progress: event.data.phase === job.phase ? job.progress : null } };
    case "JobProgress":
      return { ...jobs, [id]: { ...job, progress: event.data } };
    case "JobLog":
      return { ...jobs, [id]: { ...job, logs: [...job.logs, event.data] } };
    case "JobFinished":
      return { ...jobs, [id]: { ...job, status: event.data.status } };
    default:
      return jobs;
  }
}

/**
 * Adds jobs listed by the helper at start-up. Events may already have
 * arrived for them: what they said is kept, the listing only fills gaps.
 */
export function seedJobs(jobs: Record<string, JobView>, listed: JobSummary[]): Record<string, JobView> {
  const next = { ...jobs };
  for (const summary of listed) {
    const known = next[summary.job_id];
    next[summary.job_id] = known
      ? { ...known, phase: known.phase === "Preparing" ? summary.phase : known.phase, status: known.status ?? summary.status }
      : { ...EMPTY, phase: summary.phase, status: summary.status };
  }
  return next;
}

export function emptyJob(): JobView {
  return EMPTY;
}
