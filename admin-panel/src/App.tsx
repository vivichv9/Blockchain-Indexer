import { useEffect, useState } from "react";
import {
  getJob,
  getNodeHealth,
  listJobs,
  listNodes,
  runJobAction,
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

  const [nodes, setNodes] = useState<NodeSummary[]>([]);
  const [nodesError, setNodesError] = useState<string | null>(null);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [selectedNode, setSelectedNode] = useState<NodeHealthDetails | null>(null);
  const [nodeDetailsError, setNodeDetailsError] = useState<string | null>(null);

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
