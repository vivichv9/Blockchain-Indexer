import { useEffect, useState, type FormEvent } from "react";
import {
  createJob,
  createNode,
  getJob,
  getNodeHealth,
  listJobs,
  listNodes,
  runJobAction,
  type CreateJobPayload,
  type CreateNodePayload,
  type JobDetails,
  type JobStatus,
  type JobSummary,
  type NodeHealthDetails,
  type NodeSummary,
} from "./api";

type TabId = "jobs" | "nodes";

const tabs: Array<{ id: TabId; label: string }> = [
  { id: "jobs", label: "Jobs" },
  { id: "nodes", label: "Node Health" },
];

const refreshIntervalMs = 10_000;

export function App() {
  const [activeTab, setActiveTab] = useState<TabId>("jobs");
  const [jobs, setJobs] = useState<JobSummary[]>([]);
  const [jobsError, setJobsError] = useState<string | null>(null);
  const [selectedJob, setSelectedJob] = useState<JobDetails | null>(null);
  const [jobActionPending, setJobActionPending] = useState<string | null>(null);
  const [createJobPending, setCreateJobPending] = useState(false);
  const [jobDraft, setJobDraft] = useState<CreateJobPayload>({
    job_id: "",
    mode: "address_list",
    enabled: true,
    addresses: [],
  });
  const [addressesInput, setAddressesInput] = useState("");

  const [nodes, setNodes] = useState<NodeSummary[]>([]);
  const [nodesError, setNodesError] = useState<string | null>(null);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [selectedNode, setSelectedNode] = useState<NodeHealthDetails | null>(null);
  const [nodeDetailsError, setNodeDetailsError] = useState<string | null>(null);
  const [createNodePending, setCreateNodePending] = useState(false);
  const [nodeDraft, setNodeDraft] = useState<CreateNodePayload>({
    node_id: "",
    url: "",
    username: "",
    password: "",
    insecure_skip_verify: false,
    enabled: true,
  });

  useEffect(() => {
    void refreshJobs();
    const timer = window.setInterval(() => void refreshJobs(), refreshIntervalMs);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    void refreshNodes();
    const timer = window.setInterval(() => void refreshNodes(), refreshIntervalMs);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    if (!selectedNodeId) {
      return;
    }

    void loadNodeDetails(selectedNodeId);
  }, [selectedNodeId]);

  async function refreshJobs() {
    try {
      const data = await listJobs();
      setJobs(data);
      setJobsError(null);

      if (!selectedJob && data.length > 0) {
        void loadJobDetails(data[0].job_id);
      } else if (selectedJob) {
        const stillExists = data.some((job) => job.job_id === selectedJob.job_id);
        if (stillExists) {
          void loadJobDetails(selectedJob.job_id);
        } else {
          setSelectedJob(null);
        }
      }
    } catch (error) {
      setJobsError(asErrorMessage(error));
    }
  }

  async function refreshNodes() {
    try {
      const data = await listNodes();
      setNodes(data);
      setNodesError(null);

      const nextNodeId = selectedNodeId ?? data[0]?.node_id ?? null;
      setSelectedNodeId(nextNodeId);
      if (nextNodeId) {
        void loadNodeDetails(nextNodeId);
      }
    } catch (error) {
      setNodesError(asErrorMessage(error));
    }
  }

  async function loadJobDetails(jobId: string) {
    try {
      const details = await getJob(jobId);
      setSelectedJob(details);
    } catch (error) {
      setJobsError(asErrorMessage(error));
    }
  }

  async function loadNodeDetails(nodeId: string) {
    try {
      const details = await getNodeHealth(nodeId);
      setSelectedNode(details);
      setNodeDetailsError(null);
    } catch (error) {
      setNodeDetailsError(asErrorMessage(error));
    }
  }

  async function handleCreateNode(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setCreateNodePending(true);
    setNodesError(null);

    try {
      const details = await createNode(nodeDraft);
      setSelectedNodeId(details.node_id);
      setSelectedNode(details);
      setNodeDraft({
        node_id: "",
        url: "",
        username: "",
        password: "",
        insecure_skip_verify: false,
        enabled: true,
      });
      await refreshNodes();
    } catch (error) {
      setNodesError(asErrorMessage(error));
    } finally {
      setCreateNodePending(false);
    }
  }

  async function handleJobAction(jobId: string, action: "start" | "stop" | "pause" | "resume" | "retry") {
    setJobActionPending(`${jobId}:${action}`);
    try {
      const details = await runJobAction(jobId, action);
      setSelectedJob(details);
      await refreshJobs();
    } catch (error) {
      setJobsError(asErrorMessage(error));
    } finally {
      setJobActionPending(null);
    }
  }

  async function handleCreateJob(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setCreateJobPending(true);
    setJobsError(null);

    try {
      const payload: CreateJobPayload = {
        ...jobDraft,
        addresses:
          jobDraft.mode === "address_list"
            ? addressesInput
                .split(/\r?\n|,/)
                .map((value) => value.trim())
                .filter(Boolean)
            : [],
      };

      const details = await createJob(payload);
      setSelectedJob(details);
      setJobDraft({
        job_id: "",
        mode: "address_list",
        enabled: true,
        addresses: [],
      });
      setAddressesInput("");
      await refreshJobs();
    } catch (error) {
      setJobsError(asErrorMessage(error));
    } finally {
      setCreateJobPending(false);
    }
  }

  return (
    <div className="shell">
      <header className="hero">
        <div>
          <p className="eyebrow">Bitcoin Blockchain Indexer</p>
          <h1>Operational Console</h1>
          <p className="lede">
            Панель показывает статус индексации, состояние RPC-узла и позволяет управлять jobs через REST API.
          </p>
        </div>
        <div className="hero-card">
          <div className="hero-metric">
            <span>Jobs</span>
            <strong>{jobs.length}</strong>
          </div>
          <div className="hero-metric">
            <span>Nodes</span>
            <strong>{nodes.length}</strong>
          </div>
          <div className="hero-hint">Auth и API URL берутся только из `VITE_*` env.</div>
        </div>
      </header>

      <nav className="tabs" aria-label="Sections">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            className={tab.id === activeTab ? "tab tab-active" : "tab"}
            onClick={() => setActiveTab(tab.id)}
            type="button"
          >
            {tab.label}
          </button>
        ))}
      </nav>

      {activeTab === "jobs" ? (
        <section className="panel-grid">
          <div className="panel">
            <div className="panel-header">
              <h2>Jobs</h2>
              <button className="ghost-button" onClick={() => void refreshJobs()} type="button">
                Refresh
              </button>
            </div>
            {jobsError ? <div className="banner error">{jobsError}</div> : null}
            <form className="create-job-form" onSubmit={handleCreateJob}>
              <div className="form-grid">
                <label className="field">
                  <span>Job ID</span>
                  <input
                    onChange={(event) => setJobDraft((current) => ({ ...current, job_id: event.target.value }))}
                    placeholder="watchlist-runtime"
                    required
                    type="text"
                    value={jobDraft.job_id}
                  />
                </label>
                <label className="field">
                  <span>Mode</span>
                  <select
                    onChange={(event) =>
                      setJobDraft((current) => ({
                        ...current,
                        mode: event.target.value as CreateJobPayload["mode"],
                      }))
                    }
                    value={jobDraft.mode}
                  >
                    <option value="address_list">address_list</option>
                    <option value="all_addresses">all_addresses</option>
                  </select>
                </label>
              </div>
              <label className="field checkbox-field">
                <input
                  checked={jobDraft.enabled}
                  onChange={(event) => setJobDraft((current) => ({ ...current, enabled: event.target.checked }))}
                  type="checkbox"
                />
                <span>Start immediately after create</span>
              </label>
              <label className="field">
                <span>Addresses</span>
                <textarea
                  disabled={jobDraft.mode === "all_addresses"}
                  onChange={(event) => setAddressesInput(event.target.value)}
                  placeholder="addr1&#10;addr2"
                  rows={4}
                  value={addressesInput}
                />
              </label>
              <div className="form-hint">
                Для `address_list` укажи адреса через новую строку или запятую. Для `all_addresses` список должен быть пустым.
              </div>
              <button className="action-button" disabled={createJobPending} type="submit">
                {createJobPending ? "Creating..." : "Create Job"}
              </button>
            </form>
            <div className="table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>Job</th>
                    <th>Mode</th>
                    <th>Status</th>
                    <th>Progress</th>
                    <th>Tip</th>
                    <th>Updated</th>
                  </tr>
                </thead>
                <tbody>
                  {jobs.map((job) => (
                    <tr
                      key={job.job_id}
                      className={selectedJob?.job_id === job.job_id ? "row-selected" : ""}
                      onClick={() => void loadJobDetails(job.job_id)}
                    >
                      <td>{job.job_id}</td>
                      <td>{job.mode}</td>
                      <td><StatusBadge value={job.status} /></td>
                      <td>{job.progress_height}</td>
                      <td>{job.tip_height ?? "n/a"}</td>
                      <td>{formatDate(job.updated_at)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>

          <div className="panel">
            <div className="panel-header">
              <h2>Selected Job</h2>
              {selectedJob ? <span className="caption">{selectedJob.job_id}</span> : null}
            </div>
            {selectedJob ? (
              <>
                <div className="detail-grid">
                  <MetricCard label="Mode" value={selectedJob.mode} />
                  <MetricCard label="Status" value={selectedJob.status} />
                  <MetricCard label="Progress" value={String(selectedJob.progress_height)} />
                  <MetricCard label="Updated" value={formatDate(selectedJob.updated_at)} />
                </div>
                <div className="action-row">
                  {jobActionsForStatus(selectedJob.status).map((action) => {
                    const pending = jobActionPending === `${selectedJob.job_id}:${action}`;
                    return (
                      <button
                        key={action}
                        className="action-button"
                        disabled={pending}
                        onClick={() => void handleJobAction(selectedJob.job_id, action)}
                        type="button"
                      >
                        {pending ? "Working..." : action}
                      </button>
                    );
                  })}
                </div>
                <div className="stack">
                  <section className="detail-block">
                    <h3>Last Error</h3>
                    <pre>{selectedJob.last_error ?? "No error recorded"}</pre>
                  </section>
                  <section className="detail-block">
                    <h3>Config Snapshot</h3>
                    <pre>{JSON.stringify(selectedJob.config_snapshot, null, 2)}</pre>
                  </section>
                </div>
              </>
            ) : (
              <div className="empty-state">Select a job to inspect its state and run actions.</div>
            )}
          </div>
        </section>
      ) : (
        <section className="panel-grid">
          <div className="panel">
            <div className="panel-header">
              <h2>Nodes</h2>
              <button className="ghost-button" onClick={() => void refreshNodes()} type="button">
                Refresh
              </button>
            </div>
            {nodesError ? <div className="banner error">{nodesError}</div> : null}
            <form className="create-node-form" onSubmit={handleCreateNode}>
              <div className="form-grid">
                <label className="field">
                  <span>Node ID</span>
                  <input
                    onChange={(event) => setNodeDraft((current) => ({ ...current, node_id: event.target.value }))}
                    placeholder="btc-testnet-2"
                    required
                    type="text"
                    value={nodeDraft.node_id}
                  />
                </label>
                <label className="field">
                  <span>RPC URL</span>
                  <input
                    onChange={(event) => setNodeDraft((current) => ({ ...current, url: event.target.value }))}
                    placeholder="https://rpc.example.com"
                    required
                    type="url"
                    value={nodeDraft.url}
                  />
                </label>
              </div>
              <div className="form-grid">
                <label className="field">
                  <span>Username</span>
                  <input
                    onChange={(event) => setNodeDraft((current) => ({ ...current, username: event.target.value }))}
                    required
                    type="text"
                    value={nodeDraft.username}
                  />
                </label>
                <label className="field">
                  <span>Password</span>
                  <input
                    onChange={(event) => setNodeDraft((current) => ({ ...current, password: event.target.value }))}
                    required
                    type="password"
                    value={nodeDraft.password}
                  />
                </label>
              </div>
              <div className="form-grid">
                <label className="field checkbox-field">
                  <input
                    checked={nodeDraft.insecure_skip_verify}
                    onChange={(event) =>
                      setNodeDraft((current) => ({ ...current, insecure_skip_verify: event.target.checked }))
                    }
                    type="checkbox"
                  />
                  <span>Skip TLS verification</span>
                </label>
                <label className="field checkbox-field">
                  <input
                    checked={nodeDraft.enabled}
                    onChange={(event) => setNodeDraft((current) => ({ ...current, enabled: event.target.checked }))}
                    type="checkbox"
                  />
                  <span>Enable health polling</span>
                </label>
              </div>
              <button className="action-button" disabled={createNodePending} type="submit">
                {createNodePending ? "Adding..." : "Add Node"}
              </button>
            </form>
            <div className="node-list">
              {nodes.map((node) => (
                <button
                  key={node.node_id}
                  className={selectedNodeId === node.node_id ? "node-card node-card-active" : "node-card"}
                  onClick={() => setSelectedNodeId(node.node_id)}
                  type="button"
                >
                  <div className="node-card-head">
                    <strong>{node.node_id}</strong>
                    <StatusBadge value={node.status} />
                  </div>
                  <span>Tip: {node.tip_height}</span>
                  <span>Latency: {node.rpc_latency_ms} ms</span>
                  <span>Seen: {formatDate(node.last_seen_at)}</span>
                </button>
              ))}
            </div>
          </div>

          <div className="panel">
            <div className="panel-header">
              <h2>Node Health</h2>
              {selectedNode ? <span className="caption">{selectedNode.node_id}</span> : null}
            </div>
            {nodeDetailsError ? <div className="banner error">{nodeDetailsError}</div> : null}
            {selectedNode ? (
              <>
                <div className="detail-grid">
                  <MetricCard label="Status" value={selectedNode.status} />
                  <MetricCard label="Tip Height" value={String(selectedNode.tip_height)} />
                  <MetricCard label="Latency" value={`${selectedNode.rpc_latency_ms} ms`} />
                  <MetricCard label="Last Seen" value={formatDate(selectedNode.last_seen_at)} />
                </div>
                <div className="stack">
                  <section className="detail-block">
                    <h3>Tip Hash</h3>
                    <pre>{selectedNode.tip_hash}</pre>
                  </section>
                  <section className="detail-block">
                    <h3>Diagnostic Details</h3>
                    <pre>{JSON.stringify(selectedNode.details, null, 2)}</pre>
                  </section>
                </div>
              </>
            ) : (
              <div className="empty-state">No node health data available yet.</div>
            )}
          </div>
        </section>
      )}
    </div>
  );
}

function MetricCard(props: { label: string; value: string }) {
  return (
    <article className="metric-card">
      <span>{props.label}</span>
      <strong>{props.value}</strong>
    </article>
  );
}

function StatusBadge(props: { value: string }) {
  return <span className={`status-badge status-${props.value}`}>{props.value}</span>;
}

function jobActionsForStatus(status: JobStatus): Array<"start" | "stop" | "pause" | "resume" | "retry"> {
  switch (status) {
    case "created":
      return ["start"];
    case "running":
      return ["pause", "stop"];
    case "paused":
      return ["resume", "stop"];
    case "failed":
      return ["retry", "stop"];
    default:
      return [];
  }
}

function formatDate(value: string | null): string {
  if (!value) {
    return "n/a";
  }

  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return value;
  }

  return parsed.toLocaleString();
}

function asErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "Unknown error";
}
