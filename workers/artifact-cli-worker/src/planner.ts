import type { ArtifactInput, SourceType, WorkerFunctionPlan, WorkerPlan } from './types.js'

const DEFAULT_SOURCE_TYPE: SourceType = 'manual'

function slugify(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
}

function title(value: string): string {
  return value.replace(/[_-]+/g, ' ').replace(/\b\w/g, (m) => m.toUpperCase())
}

function inferFunctions(input: ArtifactInput): string[] {
  if (input.functions?.length) return input.functions
  const goal = `${input.goal ?? ''} ${input.source ?? ''}`.toLowerCase()
  if (goal.includes('issue') || goal.includes('linear') || goal.includes('jira')) {
    return ['list_items', 'blocked_items', 'risk_summary']
  }
  if (goal.includes('search') || goal.includes('docs')) {
    return ['search', 'get_document', 'answer_with_sources']
  }
  if (goal.includes('github') || goal.includes('repo')) {
    return ['repo_summary', 'stale_prs', 'open_issues']
  }
  return ['inspect', 'list', 'get']
}

function planFunction(namespace: string, fn: string): WorkerFunctionPlan {
  const clean = slugify(fn)
  const syncLike = clean.includes('sync') || clean.includes('refresh')
  return {
    functionId: `${namespace}::${clean}`,
    purpose: `${title(clean)} for the ${namespace} worker`,
    sideEffects: syncLike ? 'sync' : 'external-call',
    inputs: syncLike
      ? { force: 'boolean optional; bypass cache when true' }
      : { query: 'string/object; focused request payload for this function' },
    output: {
      ok: 'boolean success flag',
      data: 'function-specific result payload',
      sources: 'optional source/provenance list',
    },
  }
}

export function inspectArtifact(input: ArtifactInput) {
  const namespace = slugify(input.name || 'artifact')
  const sourceType = input.sourceType ?? DEFAULT_SOURCE_TYPE
  const functions = inferFunctions(input)
  return {
    name: input.name,
    namespace,
    sourceType,
    source: input.source,
    suggestedFunctions: functions.map((fn) => `${namespace}::${slugify(fn)}`),
    recommendation: 'Generate a narrow iii worker around the specific job, not a generic full API wrapper.',
    existingWorkersToUse: ['iii-state', 'iii-queue', 'iii-sandbox', 'iii-observability'],
  }
}

export function planWorker(input: ArtifactInput): WorkerPlan {
  const namespace = slugify(input.name || 'artifact')
  const sourceType = input.sourceType ?? DEFAULT_SOURCE_TYPE
  const functions = inferFunctions(input).map((fn) => planFunction(namespace, fn))
  return {
    workerName: `${namespace}-worker`,
    namespace,
    sourceType,
    source: input.source,
    goal: input.goal ?? `Expose focused agent-operable functions for ${input.name}.`,
    functions,
    usesWorkers: ['iii-state', 'iii-queue', 'iii-sandbox', 'iii-http', 'iii-observability'],
    notes: [
      'Keep function count small and job-specific.',
      'Prefer read-only functions unless the worker explicitly syncs or mutates external state.',
      'Persist manifests and source fingerprints through iii-state.',
      'Run generated code checks inside iii-sandbox before publishing.',
    ],
  }
}
