import { mkdtemp, rm } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { generateWorker, verifyWorker } from '../workers/artifact-cli-worker/src/generator.js'
import { planWorker } from '../workers/artifact-cli-worker/src/planner.js'

const plan = planWorker({
  name: 'hackernews',
  goal: 'focused agent access to top stories and item lookup',
  sourceType: 'docs',
  source: 'https://github.com/HackerNews/API',
  functions: ['top_stories', 'get_item', 'search_cached_stories'],
})

if (plan.namespace !== 'hackernews') throw new Error('namespace mismatch')
if (plan.functions.length !== 3) throw new Error('function count mismatch')

const dir = await mkdtemp(path.join(os.tmpdir(), 'artifact-cli-'))
try {
  const generated = await generateWorker({
    name: 'hackernews',
    sourceType: 'docs',
    source: 'https://github.com/HackerNews/API',
    outputDir: dir,
    functions: ['top_stories', 'get_item'],
  })
  if (!generated.ok) throw new Error('generate failed')
  const verified = await verifyWorker({ outputDir: dir })
  if (!verified.ok) throw new Error(`verify failed: ${verified.missingRegistrations.join(',')}`)
  console.log('smoke ok')
} finally {
  await rm(dir, { recursive: true, force: true })
}
