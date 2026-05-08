import { registerWorker } from 'iii-sdk'
import { z } from 'zod'
import { generateWorker, verifyWorker } from './generator.js'
import { inspectArtifact, planWorker } from './planner.js'

const ArtifactInput = z.object({
  name: z.string().min(1),
  goal: z.string().optional(),
  sourceType: z.enum(['openapi', 'graphql', 'har', 'docs', 'url', 'manual']).optional(),
  source: z.string().optional(),
  functions: z.array(z.string()).optional(),
  outputDir: z.string().optional(),
})

const iii = registerWorker(process.env.III_URL ?? 'ws://localhost:49134', {
  workerName: 'artifact-cli-worker',
})

iii.registerFunction(
  'artifact::inspect',
  async (payload: unknown) => inspectArtifact(ArtifactInput.parse(payload)),
  { description: 'Inspect an artifact source and suggest narrow iii worker functions.' },
)

iii.registerFunction(
  'artifact::plan_worker',
  async (payload: unknown) => planWorker(ArtifactInput.parse(payload)),
  { description: 'Create a narrow iii worker plan from an artifact description.' },
)

iii.registerFunction(
  'artifact::generate_worker',
  async (payload: unknown) => generateWorker(ArtifactInput.parse(payload)),
  { description: 'Generate a TypeScript iii worker scaffold from an artifact plan.' },
)

iii.registerFunction(
  'artifact::verify_worker',
  async (payload: unknown) => verifyWorker(z.object({ outputDir: z.string() }).parse(payload)),
  { description: 'Run structural verification on a generated artifact worker.' },
)

iii.registerFunction(
  'artifact::manifest',
  async (payload: unknown) => {
    const input = ArtifactInput.parse(payload)
    const plan = planWorker(input)
    return {
      schema: 'artifact-cli.manifest.preview.v1',
      workerName: plan.workerName,
      namespace: plan.namespace,
      functions: plan.functions,
      usesWorkers: plan.usesWorkers,
    }
  },
  { description: 'Create a manifest preview for a generated artifact worker.' },
)

console.log('artifact-cli-worker registered artifact::* functions')
