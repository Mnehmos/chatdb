import { useState, useEffect, useMemo, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useLoopStore } from '../../stores/loopStore';
import { useNavigationStore } from '../../stores/navigationStore';
import { getDagEdgesFrom } from '../../services/tauri';
import type { DagEdge } from '../../types';

interface ObligationNode {
  id: string;
  description: string;
  obligation_type: string;
  priority: number;
  confidence: number;
  status: string;
  depends_on: string | null;
  steps_spent: number;
  max_steps: number;
  superseded_by: string | null;
}

type ViewMode = 'obligations' | 'steps' | 'unified';

interface LayoutNode {
  id: string;
  label: string;
  nodeType: 'obligation' | 'step';
  x: number;
  y: number;
  color: string;
  status: string;
  sublabel?: string;
}

const NODE_W = 180;
const NODE_H = 48;
const LAYER_GAP = 70;
const COL_GAP = 200;
const PAD = 20;

function statusColor(status: string): string {
  if (status === 'open') return 'var(--yellow)';
  if (status.includes('proved') || status === 'verified') return 'var(--green)';
  if (status.includes('refuted') || status === 'rejected') return 'var(--red)';
  if (status === 'superseded' || status === 'demoted') return 'var(--text-muted)';
  return 'var(--blue)';
}

function typeAbbrev(t: string): string {
  const map: Record<string, string> = {
    'COUNT': 'CNT', 'BOUND': 'BND', 'CASE_CHECK': 'CSE', 'CONSTRUCT': 'CON',
    'CLASSIFY': 'CLS', 'IMPOSSIBILITY': 'IMP', 'SYNTHESIZE': 'SYN', 'RESOLVE': 'RES',
  };
  return map[t] || t.slice(0, 3).toUpperCase();
}

function parseDeps(depsStr: string | null): string[] {
  if (!depsStr) return [];
  try {
    const parsed = JSON.parse(depsStr);
    return Array.isArray(parsed) ? parsed : [];
  } catch { return []; }
}

interface Props {
  attemptId: string;
  problemId: string;
}

export function UnifiedDagView({ attemptId, problemId }: Props) {
  const [obligations, setObligations] = useState<ObligationNode[]>([]);
  const [dagEdges, setDagEdges] = useState<DagEdge[]>([]);
  const [viewMode, setViewMode] = useState<ViewMode>('obligations');
  const [hoveredId, setHoveredId] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(true);
  const liveObligations = useLoopStore(s => s.obligations);
  const steps = useLoopStore(s => s.steps);
  const { goToStep } = useNavigationStore();

  const attemptSteps = useMemo(() => steps.filter(s => s.attempt_id === attemptId), [steps, attemptId]);

  const fetchData = useCallback(async () => {
    try {
      const data = await invoke<ObligationNode[]>('get_obligation_graph', { attemptId });
      setObligations(data);
    } catch {
      setObligations(liveObligations.map(o => ({
        id: o.id,
        description: o.description,
        obligation_type: o.obligation_type,
        priority: o.priority,
        confidence: 0.7,
        status: o.status,
        depends_on: null,
        steps_spent: o.steps_spent || 0,
        max_steps: o.max_steps || 5,
        superseded_by: null,
      })));
    }
    // Load dag_edges for the attempt
    try {
      const edges = await getDagEdgesFrom(attemptId);
      setDagEdges(edges);
    } catch {
      setDagEdges([]);
    }
  }, [attemptId, liveObligations]);

  useEffect(() => { fetchData(); }, [liveObligations.length, fetchData]);

  const { layoutNodes, edges, svgW, svgH } = useMemo(() => {
    const layout: LayoutNode[] = [];
    const edgeList: { from: string; to: string; type: string }[] = [];

    if (viewMode === 'obligations' || viewMode === 'unified') {
      // Layout obligations using topological sort
      const idSet = new Set(obligations.map(n => n.id));
      const inDegree = new Map<string, number>();
      const children = new Map<string, string[]>();
      for (const n of obligations) {
        inDegree.set(n.id, 0);
        children.set(n.id, []);
      }
      for (const n of obligations) {
        for (const dep of parseDeps(n.depends_on)) {
          if (idSet.has(dep)) {
            inDegree.set(n.id, (inDegree.get(n.id) || 0) + 1);
            children.get(dep)!.push(n.id);
          }
        }
      }
      const layers = new Map<string, number>();
      const queue: string[] = [];
      for (const [id, deg] of inDegree) { if (deg === 0) queue.push(id); }
      let layer = 0;
      while (queue.length > 0) {
        const next: string[] = [];
        for (const id of queue) {
          layers.set(id, layer);
          for (const child of children.get(id) || []) {
            const nd = (inDegree.get(child) || 1) - 1;
            inDegree.set(child, nd);
            if (nd === 0) next.push(child);
          }
        }
        queue.length = 0;
        queue.push(...next);
        layer++;
      }
      for (const n of obligations) { if (!layers.has(n.id)) layers.set(n.id, layer); }

      // Group by layer
      const layerGroups = new Map<number, ObligationNode[]>();
      for (const n of obligations) {
        const l = layers.get(n.id) || 0;
        if (!layerGroups.has(l)) layerGroups.set(l, []);
        layerGroups.get(l)!.push(n);
      }

      for (const [, group] of layerGroups) {
        group.sort((a, b) => b.priority - a.priority);
      }

      const sortedLayers = [...layerGroups.entries()].sort((a, b) => a[0] - b[0]);
      for (const [layerIdx, group] of sortedLayers) {
        for (let col = 0; col < group.length; col++) {
          const ob = group[col];
          layout.push({
            id: ob.id,
            label: `[${typeAbbrev(ob.obligation_type)}] ${ob.description.slice(0, 22)}`,
            nodeType: 'obligation',
            x: PAD + col * COL_GAP,
            y: PAD + layerIdx * (NODE_H + LAYER_GAP),
            color: statusColor(ob.status),
            status: ob.status,
            sublabel: `${ob.steps_spent}/${ob.max_steps}`,
          });
        }
      }

      // Edges between obligations
      for (const n of obligations) {
        for (const dep of parseDeps(n.depends_on)) {
          if (idSet.has(dep)) edgeList.push({ from: dep, to: n.id, type: 'depends_on' });
        }
      }
    }

    if (viewMode === 'steps' || viewMode === 'unified') {
      const yOffset = viewMode === 'unified' ? (layout.length > 0 ? Math.max(...layout.map(n => n.y)) + NODE_H + LAYER_GAP + 20 : PAD) : PAD;

      // Layout steps in a simple grid
      const cols = 4;
      attemptSteps.forEach((step, i) => {
        layout.push({
          id: step.id,
          label: `#${step.step_number} ${step.proposal_type}`,
          nodeType: 'step',
          x: PAD + (i % cols) * COL_GAP,
          y: yOffset + Math.floor(i / cols) * (NODE_H + 20),
          color: statusColor(step.verified ? 'verified' : 'rejected'),
          status: step.verified ? 'verified' : 'rejected',
          sublabel: step.proposal_natural.slice(0, 25),
        });
      });

      // Step parent edges
      for (const step of attemptSteps) {
        if (step.parent_step_id) {
          edgeList.push({ from: step.parent_step_id, to: step.id, type: 'parent' });
        }
      }

      // dag_edges targeting steps
      if (viewMode === 'unified') {
        for (const edge of dagEdges) {
          if (edge.edge_type === 'targets' || edge.edge_type === 'closes') {
            edgeList.push({ from: edge.source_id, to: edge.target_id, type: edge.edge_type });
          }
        }
      }
    }

    const maxX = layout.length > 0 ? Math.max(...layout.map(n => n.x)) + NODE_W + PAD : 300;
    const maxY = layout.length > 0 ? Math.max(...layout.map(n => n.y)) + NODE_H + PAD : 200;

    return { layoutNodes: layout, edges: edgeList, svgW: maxX, svgH: maxY };
  }, [obligations, attemptSteps, dagEdges, viewMode]);

  if (obligations.length === 0 && attemptSteps.length === 0) return null;

  const posMap = new Map<string, { x: number; y: number }>();
  for (const ln of layoutNodes) posMap.set(ln.id, { x: ln.x, y: ln.y });

  return (
    <div className="unified-dag-section">
      <div className="dag-header" onClick={() => setExpanded(!expanded)}>
        <span>{expanded ? '\u25BC' : '\u25B6'}</span>
        <h4>DAG View</h4>
        <div className="dag-mode-buttons" onClick={e => e.stopPropagation()}>
          {(['obligations', 'steps', 'unified'] as ViewMode[]).map(mode => (
            <button
              key={mode}
              className={`btn btn-xs ${viewMode === mode ? 'btn-primary' : ''}`}
              onClick={() => setViewMode(mode)}
            >
              {mode}
            </button>
          ))}
        </div>
      </div>
      {expanded && (
        <div className="dag-container">
          <svg width={svgW} height={svgH} className="dag-svg">
            <defs>
              <marker id="dag-arrow" markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">
                <polygon points="0 0, 8 3, 0 6" fill="var(--text-muted)" />
              </marker>
            </defs>

            {/* Edges */}
            {edges.map((e, i) => {
              const from = posMap.get(e.from);
              const to = posMap.get(e.to);
              if (!from || !to) return null;
              const x1 = from.x + NODE_W / 2;
              const y1 = from.y + NODE_H;
              const x2 = to.x + NODE_W / 2;
              const y2 = to.y;
              const midY = (y1 + y2) / 2;
              const isHighlighted = hoveredId && (hoveredId === e.from || hoveredId === e.to);
              const edgeColor = e.type === 'conflicts' ? 'var(--red)' :
                e.type === 'targets' ? 'var(--cyan)' : 'var(--border)';
              return (
                <path
                  key={`e-${i}`}
                  d={`M ${x1} ${y1} C ${x1} ${midY}, ${x2} ${midY}, ${x2} ${y2}`}
                  fill="none"
                  stroke={edgeColor}
                  strokeWidth={isHighlighted ? 2 : 1}
                  opacity={isHighlighted ? 1 : 0.4}
                  strokeDasharray={e.type === 'targets' ? '4,3' : undefined}
                  markerEnd="url(#dag-arrow)"
                />
              );
            })}

            {/* Nodes */}
            {layoutNodes.map(ln => (
              <g
                key={ln.id}
                transform={`translate(${ln.x}, ${ln.y})`}
                onMouseEnter={() => setHoveredId(ln.id)}
                onMouseLeave={() => setHoveredId(null)}
                onClick={() => {
                  if (ln.nodeType === 'step') goToStep(problemId, attemptId, ln.id);
                }}
                style={{ cursor: ln.nodeType === 'step' ? 'pointer' : 'default' }}
              >
                <rect
                  width={NODE_W}
                  height={NODE_H}
                  rx={ln.nodeType === 'step' ? 2 : 4}
                  fill="var(--bg-card)"
                  stroke={ln.color}
                  strokeWidth={hoveredId === ln.id ? 2 : 1}
                />
                <text x={6} y={18} fontSize={11} fontFamily="var(--font-mono)" fill={ln.color}>
                  {ln.label.slice(0, 24)}
                </text>
                {ln.sublabel && (
                  <text x={6} y={36} fontSize={9} fontFamily="var(--font-mono)" fill="var(--text-muted)">
                    {ln.sublabel}
                  </text>
                )}
                <title>{ln.label}</title>
              </g>
            ))}
          </svg>
        </div>
      )}
    </div>
  );
}
