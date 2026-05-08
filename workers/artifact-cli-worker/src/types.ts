export type SourceType = 'openapi' | 'graphql' | 'har' | 'docs' | 'url' | 'manual'

export interface ArtifactInput {
  name: string
  goal?: string
  sourceType?: SourceType
  source?: string
  functions?: string[]
  outputDir?: string
}

export interface WorkerFunctionPlan {
  functionId: string
  purpose: string
  sideEffects: 'read' | 'write' | 'sync' | 'external-call'
  inputs: Record<string, string>
  output: Record<string, string>
}

export interface WorkerPlan {
  workerName: string
  namespace: string
  sourceType: SourceType
  source?: string
  goal: string
  functions: WorkerFunctionPlan[]
  usesWorkers: string[]
  notes: string[]
}
