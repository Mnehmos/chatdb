import { useState, useEffect, useMemo, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useLoopStore } from '../../stores/loopStore';

interface ObligationNode {
  id: string;
  description: string;
  obligation_type: string;
  priority: number;
  confidence: number;
  status: string;
  depends_on: string | null;
  decomposition_id: string | null;
  steps_spent: number;
  max_steps: number;
  escalation_level: number;
  superseded_by: string | null;
}

interface LayoutNode {
  ob: ObligationNode;
  x: number;
  y: number;
  layer: number;
  col: number;
}

interface Edge {
  from: string;
  to: string;
}

const NODE_W = 200;
const NODE_H = 56;
const LAYER_GAP = 80;
const COL_GAP = 220;
const PAD = 20;

function statusColor(status: string): string {
  if (status === 'open') return 'var(--yellow)';
  if (status === 'closed_proved' || status === 'proved') return 'var(--green)';
  if (status === 'closed_refuted' || status === 'refuted') return 'var(--red)';
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

/** Parse depends_on JSON string into array of obligation IDs */
function parseDeps(depsStr: string | null): string[] {
  if (!depsStr) return [];
  try {
    const parsed = JSON.parse(depsStr);
    return Array.isArray(parsed) ? parsed : [];
  } catch { return []; }
}

/** Assign layers via topological sort (Kahn's algorithm) */
function assignLayers(nodes: ObligationNode[]): Map<string, number> {
  const idSet = new Set(nodes.map(n => n.id));
  const inDegree = new Map<string, number>();
  const children = new Map<string, string[]>();

  for (const n of nodes) {
    inDegree.set(n.id, 0);
    children.set(n.id, []);
  }

  for (const n of nodes) {
    for (const dep of parseDeps(n.depends_on)) {
      if (idSet.has(dep)) {
        inDegree.set(n.id, (inDegree.get(n.id) || 0) + 1);
        children.get(dep)!.push(n.id);
      }
    }
  }

  const layers = new Map<string, number>();
  const queue: string[] = [];

  for (const [id, deg] of inDegree) {
    if (deg === 0) queue.push(id);
  }

  let layer = 0;
  while (queue.length > 0) {
    const nextQueue: string[] = [];
    for (const id of queue) {
      layers.set(id, layer);
      for (const child of children.get(id) || []) {
        const newDeg = (inDegree.get(child) || 1) - 1;
        inDegree.set(child, newDeg);
        if (newDeg === 0) nextQueue.push(child);
      }
    }
    queue.length = 0;
    queue.push(...nextQueue);
    layer++;
  }

  // Assign remaining (cyclic or orphan) nodes to last layer
  for (const n of nodes) {
    if (!layers.has(n.id)) layers.set(n.id, layer);
  }

  return layers;
}

export function ObligationGraph({ attemptId }: { attemptId: string }) {
  const [nodes, setNodes] = useState<ObligationNode[]>([]);
  const [hoveredId, setHoveredId] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(true);
  const liveObligations = useLoopStore(s => s.obligations);

  const fetchGraph = useCallback(async () => {
    try {
      const data = await invoke<ObligationNode[]>('get_obligation_graph', { attemptId });
      setNodes(data);
    } catch {
      // Fall back to live event data
      setNodes(liveObligations.map(o => ({
        id: o.id,
        description: o.description,
        obligation_type: o.obligation_type,
        priority: o.priority,
        confidence: 0.7,
        status: o.status,
        depends_on: null,
        decomposition_id: null,
        steps_spent: o.steps_spent || 0,
        max_steps: o.max_steps || 5,
        escalation_level: 0,
        superseded_by: null,
      })));
    }
  }, [attemptId, liveObligations]);

  // Refresh when live obligations change
  useEffect(() => { fetchGraph(); }, [liveObligations.length, fetchGraph]);

  const { layoutNodes, edges, svgW, svgH } = useMemo(() => {
    if (nodes.length === 0) return { layoutNodes: [], edges: [], svgW: 0, svgH: 0 };

    const layerMap = assignLayers(nodes);

    // Group by layer
    const layerGroups = new Map<number, ObligationNode[]>();
    for (const n of nodes) {
      const l = layerMap.get(n.id) || 0;
      if (!layerGroups.has(l)) layerGroups.set(l, []);
      layerGroups.get(l)!.push(n);
    }

    // Sort within layers: open first, then by priority desc
    for (const [, group] of layerGroups) {
      group.sort((a, b) => {
        if (a.status === 'open' && b.status !== 'open') return -1;
        if (b.status === 'open' && a.status !== 'open') return 1;
        return b.priority - a.priority;
      });
    }

    // Layout
    const layout: LayoutNode[] = [];
    let maxCols = 0;
    const sortedLayers = [...layerGroups.entries()].sort((a, b) => a[0] - b[0]);

    for (const [layerIdx, group] of sortedLayers) {
      maxCols = Math.max(maxCols, group.length);
      for (let col = 0; col < group.length; col++) {
        layout.push({
          ob: group[col],
          layer: layerIdx,
          col,
          x: PAD + col * COL_GAP,
          y: PAD + layerIdx * (NODE_H + LAYER_GAP),
        });
      }
    }

    // Edges from depends_on
    const edgeList: Edge[] = [];
    const idSet = new Set(nodes.map(n => n.id));
    for (const n of nodes) {
      for (const dep of parseDeps(n.depends_on)) {
        if (idSet.has(dep)) {
          edgeList.push({ from: dep, to: n.id });
        }
      }
    }

    const totalLayers = sortedLayers.length;
    return {
      layoutNodes: layout,
      edges: edgeList,
      svgW: Math.max(PAD * 2 + maxCols * COL_GAP, 300),
      svgH: PAD * 2 + totalLayers * (NODE_H + LAYER_GAP),
    };
  }, [nodes]);

  if (nodes.length === 0) return null;

  const posMap = new Map<string, { x: number; y: number }>();
  for (const ln of layoutNodes) posMap.set(ln.ob.id, { x: ln.x, y: ln.y });

  const openCount = nodes.filter(n => n.status === 'open').length;
  const resolvedCount = nodes.filter(n => n.status.includes('proved')).length;

  return (
    <div className="obligation-graph-section">
      <div className="obligation-graph-header" onClick={() => setExpanded(!expanded)}>
        <span className="obligation-graph-toggle">{expanded ? '\u25BC' : '\u25B6'}</span>
        <h4>Obligation Graph</h4>
        <span className="obligation-graph-counts">
          {openCount} open / {resolvedCount} resolved / {nodes.length} total
        </span>
      </div>
      {expanded && (
        <div className="obligation-graph-container">
          <svg
            width={svgW}
            height={svgH}
            className="obligation-graph-svg"
          >
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
              return (
                <path
                  key={`edge-${i}`}
                  d={`M ${x1} ${y1} C ${x1} ${midY}, ${x2} ${midY}, ${x2} ${y2}`}
                  fill="none"
                  stroke="var(--border)"
                  strokeWidth={1.5}
                  opacity={hoveredId && (hoveredId === e.from || hoveredId === e.to) ? 1 : 0.4}
                  markerEnd="url(#arrowhead)"
                />
              );
            })}
            <defs>
              <marker id="arrowhead" markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">
                <polygon points="0 0, 8 3, 0 6" fill="var(--text-muted)" />
              </marker>
            </defs>

            {/* Nodes */}
            {layoutNodes.map(ln => {
              const ob = ln.ob;
              const color = statusColor(ob.status);
              const isHovered = hoveredId === ob.id;
              const progress = ob.max_steps > 0 ? ob.steps_spent / ob.max_steps : 0;
              return (
                <g
                  key={ob.id}
                  transform={`translate(${ln.x}, ${ln.y})`}
                  onMouseEnter={() => setHoveredId(ob.id)}
                  onMouseLeave={() => setHoveredId(null)}
                  style={{ cursor: 'pointer' }}
                >
                  {/* Background */}
                  <rect
                    width={NODE_W}
                    height={NODE_H}
                    rx={4}
                    fill="var(--bg-card)"
                    stroke={color}
                    strokeWidth={isHovered ? 2 : 1}
                    opacity={ob.superseded_by ? 0.4 : 1}
                  />
                  {/* Progress bar */}
                  {ob.status === 'open' && progress > 0 && (
                    <rect
                      x={1}
                      y={NODE_H - 3}
                      width={(NODE_W - 2) * Math.min(progress, 1)}
                      height={2}
                      fill={color}
                      rx={1}
                    />
                  )}
                  {/* Type badge */}
                  <text x={6} y={15} fontSize={9} fontFamily="var(--font-mono)" fill={color} fontWeight={600}>
                    {typeAbbrev(ob.obligation_type)}
                  </text>
                  {/* Priority */}
                  <text x={NODE_W - 6} y={15} fontSize={9} fontFamily="var(--font-mono)" fill="var(--text-muted)" textAnchor="end">
                    P{(ob.priority * 10).toFixed(0)}
                  </text>
                  {/* Description (truncated) */}
                  <text x={6} y={32} fontSize={11} fill="var(--text-primary)" clipPath={`url(#clip-${ob.id})`}>
                    {ob.description.length > 28 ? ob.description.slice(0, 26) + '...' : ob.description}
                  </text>
                  {/* Status + steps */}
                  <text x={6} y={48} fontSize={9} fontFamily="var(--font-mono)" fill={color}>
                    {ob.status.replace(/_/g, ' ')}
                    {ob.status === 'open' && ` (${ob.steps_spent}/${ob.max_steps})`}
                  </text>

                  {/* Tooltip on hover */}
                  {isHovered && (
                    <title>{ob.description}{'\n'}Type: {ob.obligation_type} | Priority: {ob.priority.toFixed(2)} | Confidence: {ob.confidence.toFixed(2)}</title>
                  )}
                </g>
              );
            })}
          </svg>
        </div>
      )}
    </div>
  );
}
