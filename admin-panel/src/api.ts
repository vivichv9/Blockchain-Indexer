export type JobStatus = "created" | "running" | "paused" | "failed" | "completed";

export interface JobSummary {
  job_id: string;
  mode: string;
  status: JobStatus;
  progress_height: number;
  tip_height: number | null;
  updated_at: string | null;
  last_error: string | null;
}

export interface JobDetails extends JobSummary {
  config_snapshot: unknown;
}

export interface NodeSummary {
  node_id: string;
  status: string;
  tip_height: number;
  rpc_latency_ms: number;
  last_seen_at: string;
}

export interface NodeHealthDetails extends NodeSummary {
  tip_hash: string;
  details: unknown;
}

interface ApiErrorPayload {
  code: string;
  message: string;
  details: unknown;
}

const baseUrl = (import.meta.env.VITE_INDEXER_API_BASE_URL as string | undefined)?.replace(/\/$/, "") ?? "";
const username = (import.meta.env.VITE_INDEXER_API_USERNAME as string | undefined) ?? "";
const password = (import.meta.env.VITE_INDEXER_API_PASSWORD as string | undefined) ?? "";

function authHeader(): string {
  return `Basic ${btoa(`${username}:${password}`)}`;
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  if (!baseUrl) {
    throw new Error("VITE_INDEXER_API_BASE_URL is not configured");
  }

  const response = await fetch(`${baseUrl}${path}`, {
    ...init,
    headers: {
      Authorization: authHeader(),
      "Content-Type": "application/json",
      ...(init?.headers ?? {}),
    },
  });

  if (!response.ok) {
    const errorBody = (await safeJson<ApiErrorPayload>(response)) ?? {
      code: "HTTP_ERROR",
      message: response.statusText,
      details: {},
    };
    throw new Error(`${errorBody.code}: ${errorBody.message}`);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return response.json() as Promise<T>;
}

async function safeJson<T>(response: Response): Promise<T | null> {
  try {
    return (await response.json()) as T;
  } catch {
    return null;
  }
}

export async function listJobs(): Promise<JobSummary[]> {
  const response = await request<{ items: JobSummary[] }>("/v1/jobs");
  return response.items;
}

export async function getJob(jobId: string): Promise<JobDetails> {
  const response = await request<{ item: JobDetails }>(`/v1/jobs/${jobId}`);
  return response.item;
}

export async function runJobAction(jobId: string, action: "start" | "stop" | "pause" | "resume" | "retry"): Promise<JobDetails> {
  const response = await request<{ item: JobDetails }>(`/v1/jobs/${jobId}/${action}`, { method: "POST" });
  return response.item;
}

export async function listNodes(): Promise<NodeSummary[]> {
  const response = await request<{ items: NodeSummary[] }>("/v1/nodes");
  return response.items;
}

export async function getNodeHealth(nodeId: string): Promise<NodeHealthDetails> {
  const response = await request<{ item: NodeHealthDetails }>(`/v1/nodes/${nodeId}/health`);
  return response.item;
}
